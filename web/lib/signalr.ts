// SPDX-License-Identifier: Apache-2.0
import { HubConnection, HubConnectionBuilder, LogLevel } from "@microsoft/signalr";

// The app session cookie is first-party (same origin via Next rewrites), so the
// hub handshake carries it automatically — no access_token query needed here.
export function createHub(): HubConnection {
  return new HubConnectionBuilder()
    .withUrl("/hubs/player")
    .withAutomaticReconnect()
    .configureLogging(LogLevel.Warning)
    .build();
}
