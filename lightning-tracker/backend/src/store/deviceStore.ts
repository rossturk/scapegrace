import { readFileSync, writeFileSync, existsSync } from "node:fs";
import type { Device } from "../types.js";

/**
 * Registry of devices and their alert geofences.
 *
 * Persists to a JSON file so registrations survive a restart. For production
 * this would be a real database; the interface is intentionally small so that
 * swap is local.
 */
export class DeviceStore {
  private devices = new Map<string, Device>();

  constructor(private readonly path?: string) {
    if (path && existsSync(path)) {
      try {
        const raw = JSON.parse(readFileSync(path, "utf8")) as Device[];
        for (const d of raw) this.devices.set(d.token, d);
      } catch {
        // Corrupt file: start empty rather than crash.
      }
    }
  }

  upsert(device: Device): Device {
    const merged: Device = { ...device, updatedAt: Date.now() };
    this.devices.set(device.token, merged);
    this.persist();
    return merged;
  }

  remove(token: string): boolean {
    const existed = this.devices.delete(token);
    if (existed) this.persist();
    return existed;
  }

  get(token: string): Device | undefined {
    return this.devices.get(token);
  }

  all(): Device[] {
    return [...this.devices.values()];
  }

  private persist(): void {
    if (!this.path) return;
    writeFileSync(this.path, JSON.stringify(this.all(), null, 2));
  }
}
