"use strict";
const net = require("node:net");
const { randomBytes, timingSafeEqual } = require("node:crypto");
const { normalizeOptions } = require("./options");
const MAX_FRAME = 1024 * 1024;
const KINDS = new Set(["ecs", "states", "signals", "pools", "stores"]);

function validSnapshot(value) {
  return value?.version === 1 && Number.isSafeInteger(value.sequence) && value.sequence > 0 &&
    Array.isArray(value.objects) && value.objects.length <= 128 && value.objects.every(item =>
      Number.isSafeInteger(item?.id) && KINDS.has(item.kind) && typeof item.label === "string" &&
      item.data && typeof item.data === "object" && !Array.isArray(item.data));
}

async function listen({ onSnapshot, onStatus, updateInterval }) {
  let interval = normalizeOptions({ updateInterval }).updateInterval;
  const token = randomBytes(32).toString("hex");
  const sockets = new Set();
  let active, disposed = false, paused = false;
  const server = net.createServer(socket => {
    if (sockets.size >= 4 || disposed) { socket.destroy(); return; }
    sockets.add(socket);
    socket.setNoDelay(true);
    let buffer = Buffer.alloc(0), authenticated = false;
    const timeout = setTimeout(() => { if (!authenticated) socket.destroy(); }, 3000);
    timeout.unref();
    socket.on("error", () => {});
    socket.on("close", () => {
      clearTimeout(timeout); sockets.delete(socket);
      if (active === socket) { active = undefined; if (!disposed) onStatus?.("disconnected"); }
    });
    socket.on("data", chunk => {
      if (buffer.length + chunk.length > MAX_FRAME) { socket.destroy(); return; }
      buffer = Buffer.concat([buffer, chunk]);
      let index, frames = 0;
      while ((index = buffer.indexOf(10)) !== -1) {
        if (++frames > 32) { socket.destroy(); return; }
        const line = buffer.subarray(0, index); buffer = buffer.subarray(index + 1);
        let message;
        try { message = JSON.parse(line.toString("utf8")); } catch { socket.destroy(); return; }
        if (!authenticated) {
          const provided = Buffer.from(typeof message?.token === "string" ? message.token : "");
          if (message?.type !== "hello" || message.version !== 1 || provided.length !== token.length ||
              !timingSafeEqual(provided, Buffer.from(token))) { socket.destroy(); return; }
          authenticated = true; clearTimeout(timeout);
          active?.destroy(); active = socket;
          socket.write(`interval ${interval}\n${paused ? "pause" : "resume"}\n`);
          onStatus?.("connected");
        } else {
          if (active !== socket || !validSnapshot(message)) { socket.destroy(); return; }
          onSnapshot?.(message);
        }
      }
    });
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => { server.removeListener("error", reject); resolve(); });
  });
  server.on("error", () => onStatus?.("disconnected"));
  server.unref();
  onStatus?.("waiting");
  return {
    port: server.address().port, token,
    pause(value) { paused = value; if (active && active.writableLength < 1024) active.write(value ? "pause\n" : "resume\n"); },
    setInterval(value) { interval = normalizeOptions({ updateInterval: value }).updateInterval; if (active && active.writableLength < 1024) active.write(`interval ${interval}\n`); },
    stop() { if (active && active.writableLength < 1024) active.end("stop\n"); },
    refresh() { if (active && active.writableLength < 1024) active.write("snapshot\n"); },
    dispose() { disposed = true; for (const socket of sockets) socket.destroy(); server.close(); },
  };
}
module.exports = { listen, validSnapshot, MAX_FRAME };
