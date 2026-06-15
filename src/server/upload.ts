/**
 * Handler for the /upload endpoint.
 * Receives a file via multipart/form-data, stores it temporarily, and returns a file ID.
 */

import type http from "node:http";
import fs from "node:fs";
import Busboy from "busboy";
import { UploadStore, MAX_FILE_SIZE, ALLOWED_MIME_TYPES } from "../services/upload_store.js";
import { logger } from "../utils/logger.js";

/** Singleton UploadStore instance */
let uploadStore: UploadStore | null = null;

export function setUploadStore(store: UploadStore): void {
  uploadStore = store;
}

export function getUploadStore(): UploadStore | null {
  return uploadStore;
}

/** Request handler for the /upload endpoint */
export async function handleUpload(req: http.IncomingMessage, res: http.ServerResponse): Promise<void> {
  if (!uploadStore) {
    res.writeHead(500, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: "UploadStore not initialized" }));
    return;
  }

  const contentType = req.headers["content-type"] ?? "";
  if (!contentType.includes("multipart/form-data")) {
    res.writeHead(400, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: "Content-Type must be multipart/form-data" }));
    return;
  }

  return new Promise<void>((resolve) => {
    let fileProcessed = false;
    let fileId: string | null = null;
    let totalBytes = 0;
    let sizeLimitExceeded = false;

    const busboy = Busboy({
      headers: req.headers,
      limits: { files: 1, fileSize: MAX_FILE_SIZE },
    });

    busboy.on("file", (_fieldname, fileStream, info) => {
      if (fileProcessed) {
        fileStream.resume(); // Discard any additional files
        return;
      }
      fileProcessed = true;

      const { filename, mimeType } = info;

      // Validate the MIME type
      if (!UploadStore.isAllowedMimeType(mimeType)) {
        fileStream.resume();
        res.writeHead(400, { "Content-Type": "application/json" });
        res.end(JSON.stringify({
          error: `Unsupported file type: ${mimeType}`,
          supported_types: [...ALLOWED_MIME_TYPES],
        }));
        resolve();
        return;
      }

      const registered = uploadStore!.register(filename ?? "upload", mimeType);
      fileId = registered.fileId;
      const writeStream = fs.createWriteStream(registered.storagePath);

      fileStream.on("data", (chunk: Buffer) => {
        totalBytes += chunk.length;
      });

      fileStream.on("limit", () => {
        // busboy detected that the size limit was exceeded
        sizeLimitExceeded = true;
        writeStream.destroy();
        void uploadStore!.rollback(registered.fileId);
        res.writeHead(413, { "Content-Type": "application/json" });
        res.end(JSON.stringify({
          error: `File too large: exceeds max size of ${MAX_FILE_SIZE} bytes`,
        }));
        resolve();
      });

      fileStream.pipe(writeStream);

      writeStream.on("finish", () => {
        if (sizeLimitExceeded) return;
        uploadStore!.updateSize(registered.fileId, totalBytes);
        const entry = uploadStore!.get(registered.fileId);
        if (!entry) return;

        logger.info({
          fileId: entry.fileId,
          originalName: entry.originalName,
          mimeType: entry.mimeType,
          sizeBytes: entry.sizeBytes,
        }, "File upload succeeded");

        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({
          file_id: entry.fileId,
          original_name: entry.originalName,
          mime_type: entry.mimeType,
          size_bytes: entry.sizeBytes,
          expires_at: entry.expiresAt.toISOString(),
        }));
        resolve();
      });

      writeStream.on("error", (err) => {
        if (sizeLimitExceeded) return;
        logger.error({ err, fileId: registered.fileId }, "File write error");
        void uploadStore!.rollback(registered.fileId);
        res.writeHead(500, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "Failed to save uploaded file" }));
        resolve();
      });
    });

    busboy.on("finish", () => {
      if (!fileProcessed && !sizeLimitExceeded) {
        // No file was attached
        res.writeHead(400, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "No file provided" }));
        resolve();
      }
    });

    busboy.on("error", (err) => {
      logger.error({ err }, "multipart parse error");
      if (fileId) {
        void uploadStore!.rollback(fileId);
      }
      if (!res.headersSent) {
        res.writeHead(400, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "Invalid multipart request" }));
      }
      resolve();
    });

    req.pipe(busboy);
  });
}
