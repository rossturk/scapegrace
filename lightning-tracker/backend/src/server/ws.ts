import { WebSocketServer, WebSocket } from "ws";
import type { Server } from "node:http";
import type { Strike } from "../types.js";
import { bboxContains, type BBox } from "../geo.js";

interface Client {
  socket: WebSocket;
  bbox?: BBox;
}

/**
 * Live strike fan-out to connected app clients. A client may send
 * `{ "type": "subscribe", "bbox": [minLat, minLon, maxLat, maxLon] }`
 * to receive only strikes inside its current map viewport.
 */
export class StrikeSocket {
  private wss: WebSocketServer;
  private clients = new Set<Client>();

  constructor(server: Server, path = "/ws") {
    this.wss = new WebSocketServer({ server, path });
    this.wss.on("connection", (socket) => {
      const client: Client = { socket };
      this.clients.add(client);
      socket.on("message", (raw) => {
        try {
          const msg = JSON.parse(raw.toString());
          if (msg.type === "subscribe" && Array.isArray(msg.bbox)) {
            const [minLat, minLon, maxLat, maxLon] = msg.bbox;
            client.bbox = { minLat, minLon, maxLat, maxLon };
          }
        } catch {
          // Ignore malformed control frames.
        }
      });
      socket.on("close", () => this.clients.delete(client));
      socket.on("error", () => this.clients.delete(client));
    });
  }

  broadcast(strike: Strike): void {
    const frame = JSON.stringify({ type: "strike", strike });
    for (const client of this.clients) {
      if (client.socket.readyState !== WebSocket.OPEN) continue;
      if (client.bbox && !bboxContains(client.bbox, strike.lat, strike.lon))
        continue;
      client.socket.send(frame);
    }
  }

  get clientCount(): number {
    return this.clients.size;
  }
}
