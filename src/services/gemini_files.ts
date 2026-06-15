/**
 * Gemini File API operations module (upload / list / status / delete).
 */

import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { getSharedGenAI, TIMEOUT_MS } from "./gemini_client.js";
import { withTimeout } from "../utils/telemetry.js";

/** Determines whether the file path contains non-ASCII characters */
function hasNonAscii(s: string): boolean {
  return /[^\x00-\x7F]/.test(s);
}

/** Common fields of a File API response */
export interface FileEntry {
  name: string | undefined;
  display_name: string | undefined;
  mime_type: string | undefined;
  state: string;
  uri: string | undefined;
  size_bytes: string | undefined;
  expiration_time: string | undefined | null;
}

/** Shared helper that converts a Gemini File API response into a FileEntry */
export function toFileEntry(file: { name?: string; displayName?: string; mimeType?: string; state?: string; uri?: string; sizeBytes?: string; expirationTime?: string | null }): FileEntry {
  return {
    name: file.name,
    display_name: file.displayName,
    mime_type: file.mimeType,
    state: file.state ?? "UNKNOWN",
    uri: file.uri,
    size_bytes: file.sizeBytes,
    expiration_time: file.expirationTime ?? null,
  };
}

export async function uploadFile(
  filePath: string,
  displayName?: string
): Promise<FileEntry> {
  const genai = await getSharedGenAI();
  // The @google/genai SDK uses the file path in an HTTP header (Content-Disposition), so paths
  // containing non-ASCII characters (e.g. Japanese) trigger a ByteString error.
  // Workaround: copy to a temporary file and preserve the original filename as displayName.
  const needsTempCopy = hasNonAscii(filePath);
  let actualPath = filePath;
  let tmpPath: string | undefined;

  if (needsTempCopy) {
    const ext = path.extname(filePath);
    tmpPath = path.join(os.tmpdir(), `gemini_upload_${Date.now()}${ext}`);
    await fs.copyFile(filePath, tmpPath);
    actualPath = tmpPath;
  }

  const resolvedDisplayName = displayName ?? (needsTempCopy ? path.basename(filePath) : undefined);

  try {
    const file = await withTimeout(
      genai.files.upload({
        file: actualPath,
        ...(resolvedDisplayName ? { config: { displayName: resolvedDisplayName } } : {}),
      }),
      TIMEOUT_MS * 2
    );
    return toFileEntry(file);
  } finally {
    if (tmpPath) {
      await fs.unlink(tmpPath).catch(() => {});
    }
  }
}

export async function listFiles(): Promise<FileEntry[]> {
  const genai = await getSharedGenAI();
  const files: FileEntry[] = [];
  // Wrap the entire list retrieval in withTimeout to apply timeout control consistent with other operations
  await withTimeout(
    (async () => {
      const pager = await genai.files.list();
      for await (const file of pager) {
        files.push(toFileEntry(file));
      }
    })(),
    TIMEOUT_MS,
  );
  return files;
}

export async function getFileStatus(fileName: string): Promise<FileEntry> {
  const genai = await getSharedGenAI();
  const file = await withTimeout(genai.files.get({ name: fileName }), TIMEOUT_MS);
  return toFileEntry(file);
}

export async function deleteFile(fileName: string): Promise<void> {
  const genai = await getSharedGenAI();
  await withTimeout(genai.files.delete({ name: fileName }), TIMEOUT_MS);
}
