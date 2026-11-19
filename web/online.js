// オンライン対戦: 2 つのブラウザを WebRTC のデータチャネルで直接つなぐ。
// 仲介サーバーを使わず、接続情報 (SDP) を「招待コード」「返信コード」として手でやり取りする。
// インターネット越しにつなぐため、自分の外向きアドレスを調べる公開 STUN サーバーだけは使う
// (STUN には接続情報しか送らず、手の内容はブラウザ同士で直接やり取りする)。

const ICE_SERVERS = [{ urls: ["stun:stun.l.google.com:19302", "stun:stun1.l.google.com:19302"] }];
const ICE_TIMEOUT_MS = 5000;

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

/**
 * 相手との接続。onMessage(obj) で受信したメッセージ、onState("open" | "closed") で接続状態を受け取る。
 * 招待する側: createInvite() → (相手の返信コードで) acceptReply(code)
 * 招待された側: joinWithInvite(code) → 返信コードを相手に送る
 */
export class Link {
  constructor(onMessage, onState) {
    this.onMessage = onMessage;
    this.onState = onState;
    this.pc = null;
    this.dc = null;
  }

  get connected() {
    return this.dc?.readyState === "open";
  }

  #newPeer() {
    this.close();
    const pc = new RTCPeerConnection({ iceServers: ICE_SERVERS });
    pc.addEventListener("connectionstatechange", () => {
      if (pc !== this.pc) return;
      if (pc.connectionState === "failed" || pc.connectionState === "closed") this.onState("closed");
    });
    this.pc = pc;
    return pc;
  }

  #bind(dc) {
    this.dc = dc;
    dc.onopen = () => this.onState("open");
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
    this.#bind(pc.createDataChannel("qchess"));
    await pc.setLocalDescription(await pc.createOffer());
    await waitIce(pc);
    return encode({ app: "qchess", kind: "invite", sdp: pc.localDescription.sdp });
  }

  async acceptReply(code) {
    const msg = await decode(code);
    if (msg.app !== "qchess" || msg.kind !== "reply") throw new Error("not a reply code");
    if (!this.pc) throw new Error("create an invite code first");
    await this.pc.setRemoteDescription({ type: "answer", sdp: msg.sdp });
  }

  async joinWithInvite(code) {
    const msg = await decode(code);
    if (msg.app !== "qchess" || msg.kind !== "invite") throw new Error("not an invite code");
    const pc = this.#newPeer();
    pc.ondatachannel = (ev) => this.#bind(ev.channel);
    await pc.setRemoteDescription({ type: "offer", sdp: msg.sdp });
    await pc.setLocalDescription(await pc.createAnswer());
    await waitIce(pc);
    return encode({ app: "qchess", kind: "reply", sdp: pc.localDescription.sdp });
  }

  send(obj) {
    if (this.connected) this.dc.send(JSON.stringify(obj));
  }

  close() {
    const { pc, dc } = this;
    this.pc = null;
    this.dc = null;
    try {
      dc?.close();
      pc?.close();
    } catch {}
  }
}
