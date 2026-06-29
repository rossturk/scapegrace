import { WebSocket } from "ws";
import type { Feed } from "./feed.js";
import type { Strike } from "../types.js";

/**
 * EXPERIMENTAL adapter for the Blitzortung community WebSocket.
 *
 * Blitzortung publishes strikes over a set of rotating WebSocket servers
 * (ws1..wsN.blitzortung.org). After connecting you send a small JSON init
 * frame; the server then streams strike messages whose payload is compressed
 * with a tiny LZW-style scheme (the same one the public live map uses). We
 * decode it, JSON.parse it, and normalize to a Strike.
 *
 * The protocol is community-reverse-engineered and undocumented, so endpoints
 * and the message shape can change without notice. Treat this as best-effort:
 * the simulator feed is the dependable default, and per Blitzortung's data
 * policy this relay must be self-hosted and used non-commercially.
 */
export class BlitzortungFeed implements Feed {
  readonly name = "blitzortung";
  private ws?: WebSocket;
  private closed = false;
  private hosts = [
    "wss://ws1.blitzortung.org/",
    "wss://ws7.blitzortung.org/",
    "wss://ws8.blitzortung.org/",
  ];
  private hostIdx = 0;

  async start(onStrike: (s: Strike) => void): Promise<void> {
    this.connect(onStrike);
  }

  private connect(onStrike: (s: Strike) => void): void {
    if (this.closed) return;
    const url = this.hosts[this.hostIdx % this.hosts.length]!;
    this.hostIdx++;
    const ws = new WebSocket(url);
    this.ws = ws;

    ws.on("open", () => {
      // Subscribe to the global stream. The live map sends a viewport; the
      // empty/global request keeps this adapter region-agnostic.
      ws.send(JSON.stringify({ a: 111 }));
    });

    ws.on("message", (data) => {
      try {
        const text = decode(data.toString());
        const msg = JSON.parse(text) as {
          time?: number;
          lat?: number;
          lon?: number;
          sig?: unknown;
        };
        if (typeof msg.lat !== "number" || typeof msg.lon !== "number") return;
        // Blitzortung timestamps are nanoseconds since epoch.
        const t =
          typeof msg.time === "number" ? Math.round(msg.time / 1e6) : Date.now();
        onStrike({
          id: `bo-${t}-${msg.lat.toFixed(4)}-${msg.lon.toFixed(4)}`,
          lat: msg.lat,
          lon: msg.lon,
          t,
        });
      } catch {
        // Heartbeats and non-strike frames land here; ignore them.
      }
    });

    const reconnect = () => {
      if (this.closed) return;
      setTimeout(() => this.connect(onStrike), 3000);
    };
    ws.on("close", reconnect);
    ws.on("error", () => ws.close());
  }

  async stop(): Promise<void> {
    this.closed = true;
    this.ws?.close();
  }
}

/**
 * Blitzortung's LZW-style decompression, as used by the public live map.
 * Expands the streamed payload back into a JSON string.
 */
function decode(input: string): string {
  const dict: Record<number, string> = {};
  const chars = input.split("");
  let currChar = chars[0]!;
  let oldPhrase = currChar;
  const out: string[] = [currChar];
  let code = 256;
  let phrase: string;
  for (let i = 1; i < chars.length; i++) {
    const currCode = chars[i]!.charCodeAt(0);
    if (currCode < 256) {
      phrase = chars[i]!;
    } else {
      phrase = dict[currCode] ? dict[currCode]! : oldPhrase + currChar;
    }
    out.push(phrase);
    currChar = phrase.charAt(0);
    dict[code] = oldPhrase + currChar;
    code++;
    oldPhrase = phrase;
  }
  return out.join("");
}
