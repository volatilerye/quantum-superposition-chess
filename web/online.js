// オンライン対戦: 2 つのブラウザを WebRTC のデータチャネルで直接つなぐ。
// 仲介サーバーを使わず、接続情報 (SDP) を「招待リンク」「返信リンク」に入れてやり取りする。
// インターネット越しにつなぐため、自分の外向きアドレスを調べる Google の公開 STUN サーバーだけは使う
// (STUN には接続情報しか送らず、手の内容はブラウザ同士で直接やり取りする)。TURN (中継) は使わない。

const ICE_SERVERS = [{ urls: ["stun:stun.l.google.com:19302", "stun:stun1.l.google.com:19302"] }];
const ICE_TIMEOUT_MS = 5000;
/** 招待・返信リンクを作ってから、つながるまで待つ時間 (リンクをチャットで送り合う時間を見込む) */
export const CONNECT_TIMEOUT_MS = 10 * 60 * 1000;
/** つながる前に WebRTC が「失敗」と判断したとき、相手の接続候補を登録し直して試し続ける間隔 */
const RETRY_MS = 5000;

/** SDP の中の接続候補 (a=candidate:…) を addIceCandidate に渡せる形で取り出す */
function candidatesOf(sdp) {
  const mid = (sdp.match(/^a=mid:(\S+)/m) || [])[1] ?? "0";
  return [...sdp.matchAll(/^a=(candidate:[^\r\n]+)/gm)].map((m) => ({ candidate: m[1], sdpMid: mid, sdpMLineIndex: 0 }));
}

/** オブジェクト → 圧縮した base64url 文字列 */
async function encode(obj) {
  const bytes = new TextEncoder().encode(JSON.stringify(obj));
  const stream = new Blob([bytes]).stream().pipeThrough(new CompressionStream("deflate-raw"));
  const buf = new Uint8Array(await new Response(stream).arrayBuffer());
  let s = "";
  for (const b of buf) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** encode の逆。壊れたコードなら例外 */
async function decode(code) {
  const b64 = code.trim().replace(/\s+/g, "").replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(b64 + "=".repeat((4 - (b64.length % 4)) % 4));
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
  return JSON.parse(await new Response(stream).text());
}

/** 接続候補 (ICE candidate) を集め終わるまで待つ (コードに全部入れるため。時間切れなら集まった分で進む) */
function waitIce(pc) {
  return new Promise((resolve) => {
    if (pc.iceGatheringState === "complete") return resolve();
    const timer = setTimeout(resolve, ICE_TIMEOUT_MS);
    pc.addEventListener("icegatheringstatechange", () => {
      if (pc.iceGatheringState === "complete") {
        clearTimeout(timer);
        resolve();
      }
    });
  });
}

/** 招待・返信コードに入っている招待の id (読めなければ null) */
export async function inviteIdOf(code) {
  try {
    return (await decode(code)).id ?? null;
  } catch {
    return null;
  }
}

/**
 * 相手との接続。onMessage(obj) で受信したメッセージ、onState(st) で接続状態を受け取る:
 * "open" (つながった) / "unstable" (一時的に通信できない。相手のブラウザが裏に回った等) /
 * "stable" (不安定から戻った) / "closed" (切れた。もう戻らない) /
 * "timeout" (CONNECT_TIMEOUT_MS たってもつながらなかった)。
 * つながる前に WebRTC が「失敗」と判断しても (相手がまだリンクを開いていないだけのことが多いので) 諦めず、
 * 相手の接続候補を登録し直して、相手が来るのを待ち続ける
 * 招待する側: createInvite() → (相手の返信コードで) acceptReply(code)
 * 招待された側: joinWithInvite(code) → 返信コードを相手に送る
 * 招待コードには招待ごとの id が入り、返信コードにも同じ id が入る (inviteIdOf で取り出せる)
 */
export class Link {
  constructor(onMessage, onState) {
    this.onMessage = onMessage;
    this.onState = onState;
    this.pc = null;
    this.dc = null;
    this.opened = false;
    this.remote = []; // 相手の接続候補 (登録し直すため)
    this.timers = [];
  }

  get connected() {
    return this.dc?.readyState === "open";
  }

  #newPeer() {
    this.close();
    const pc = new RTCPeerConnection({ iceServers: ICE_SERVERS });
    pc.addEventListener("connectionstatechange", () => {
      if (pc !== this.pc) return;
      const st = pc.connectionState;
      // "disconnected" は一時的なもので、相手が戻れば自動で "connected" に戻る
      if (st === "disconnected" && this.connected) this.onState("unstable");
      else if (st === "connected" && this.connected) this.onState("stable");
      else if (st === "closed" || (st === "failed" && this.opened)) this.onState("closed");
    });
    // つながる前の「失敗」は、相手がまだ来ていないだけ。相手の接続候補を登録し直して試し続ける
    this.timers.push(
      setInterval(() => {
        if (pc !== this.pc || this.opened) return;
        if (pc.iceConnectionState === "failed" || pc.connectionState === "failed") {
          for (const c of this.remote) pc.addIceCandidate(c).catch(() => {});
        }
      }, RETRY_MS),
      setTimeout(() => {
        if (pc !== this.pc || this.opened) return;
        this.close();
        this.onState("timeout");
      }, CONNECT_TIMEOUT_MS),
    );
    this.pc = pc;
    return pc;
  }

  #bind(dc) {
    this.dc = dc;
    dc.onopen = () => {
      this.opened = true;
      this.onState("open");
    };
    dc.onclose = () => {
      if (dc === this.dc) this.onState("closed");
    };
    dc.onmessage = (ev) => {
      try {
        this.onMessage(JSON.parse(ev.data));
      } catch {}
    };
  }

  async createInvite() {
    const pc = this.#newPeer();
    this.id = Math.random().toString(36).slice(2, 10);
    this.#bind(pc.createDataChannel("qchess"));
    await pc.setLocalDescription(await pc.createOffer());
    await waitIce(pc);
    return encode({ app: "qchess", kind: "invite", id: this.id, sdp: pc.localDescription.sdp });
  }

  async acceptReply(code) {
    const msg = await decode(code);
    if (msg.app !== "qchess" || msg.kind !== "reply") throw new Error("not a reply code");
    if (!this.pc) throw new Error("create an invite first");
    if (msg.id !== this.id) throw new Error("this reply is for another invite");
    await this.pc.setRemoteDescription({ type: "answer", sdp: msg.sdp });
    this.remote = candidatesOf(msg.sdp);
  }

  async joinWithInvite(code) {
    const msg = await decode(code);
    if (msg.app !== "qchess" || msg.kind !== "invite") throw new Error("not an invite code");
    const pc = this.#newPeer();
    this.id = msg.id;
    pc.ondatachannel = (ev) => this.#bind(ev.channel);
    await pc.setRemoteDescription({ type: "offer", sdp: msg.sdp });
    this.remote = candidatesOf(msg.sdp);
    await pc.setLocalDescription(await pc.createAnswer());
    await waitIce(pc);
    return encode({ app: "qchess", kind: "reply", id: msg.id, sdp: pc.localDescription.sdp });
  }

  send(obj) {
    if (this.connected) this.dc.send(JSON.stringify(obj));
  }

  close() {
    const { pc, dc } = this;
    this.pc = null;
    this.dc = null;
    for (const t of this.timers) clearTimeout(t);
    this.timers = [];
    try {
      dc?.close();
      pc?.close();
    } catch {}
  }
}
