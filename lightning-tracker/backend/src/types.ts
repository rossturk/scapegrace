/** A single geolocated lightning strike, normalized across feed providers. */
export interface Strike {
  id: string;
  lat: number;
  lon: number;
  /** Epoch milliseconds. */
  t: number;
  /** Optional signal strength / peak current proxy, if the feed provides one. */
  intensity?: number;
}

/** A place the user wants to be alerted about. */
export interface Geofence {
  id: string;
  name: string;
  lat: number;
  lon: number;
  radiusKm: number;
  /**
   * "first" => one alert per storm (then silent for STORM_WINDOW).
   * "every" => alert on every strike, rate-limited by ALERT_COOLDOWN.
   */
  mode: "first" | "every";
}

/** Quiet hours expressed in whole hours, 0-23, in the device's UTC offset. */
export interface QuietHours {
  /** Inclusive start hour. */
  start: number;
  /** Exclusive end hour. May wrap past midnight (e.g. 22 -> 7). */
  end: number;
  /** Minutes offset from UTC for the device's local time (e.g. -300 for EST). */
  utcOffsetMin?: number;
}

/** A registered iOS device and its alert preferences. */
export interface Device {
  /** APNs device token (hex string). */
  token: string;
  platform: "ios";
  places: Geofence[];
  quietHours?: QuietHours;
  muted?: boolean;
  updatedAt: number;
}

/** A strike matched against a device's geofence, ready to push. */
export interface AlertMatch {
  device: Device;
  place: Geofence;
  strike: Strike;
  distanceKm: number;
  bearingDeg: number;
  compass: string;
}
