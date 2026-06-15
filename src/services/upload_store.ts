/**
 * Temporary file store.
 * Manages metadata of uploaded files in an in-memory Map and automatically
 * deletes them after the TTL elapses.
 */

import fs from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { randomUUID } from "node:crypto";
import { logger } from "../utils/logger.js";

/** TTL for uploaded files (5 minutes) */
const FILE_TTL_MS = 5 * 60 * 1000;

/** Cleanup interval (60 seconds) */
const CLEANUP_INTERVAL_MS = 60 * 1000;

/** Maximum file size (50MB) */
export const MAX_FILE_SIZE = 50 * 1024 * 1024;

/** Supported MIME types */
export const ALLOWED_MIME_TYPES = new Set([
  // Images
  "image/jpeg",
  "image/png",
  "image/gif",
  "image/webp",
  // PDF
  "application/pdf",
  // Video
  "video/mp4",
  "video/mpeg",
  "video/quicktime",
  "video/webm",
  // Audio
  "audio/mpeg",
  "audio/wav",
  "audio/ogg",
  "audio/flac",
]);

/** Metadata of an uploaded file */
export interface UploadedFile {
  fileId: string;
  originalName: string;
  mimeType: string;
  sizeBytes: number;
  uploadedAt: Date;
  expiresAt: Date;
  storagePath: string;
}

export class UploadStore {
  private readonly files = new Map<string, UploadedFile>();
  private cleanupTimer: ReturnType<typeof setInterval> | null = null;
  private readonly baseDir: string;

  constructor(baseDir?: string) {
    this.baseDir = baseDir ?? path.join(os.tmpdir(), "mcp-gemini-uploads");
  }

  /** Initializes the store, creating the temporary directory and starting the cleanup timer */
  async init(): Promise<void> {
    await fs.mkdir(this.baseDir, { recursive: true });
    this.cleanupTimer = setInterval(() => {
      void this.cleanup();
    }, CLEANUP_INTERVAL_MS);
    // Prevent the timer from blocking process exit
    this.cleanupTimer.unref();
    logger.info({ baseDir: this.baseDir }, "UploadStore initialized");
  }

  /** Registers a new file entry and returns its storage path */
  register(originalName: string, mimeType: string): { fileId: string; storagePath: string } {
    const fileId = randomUUID();
    const ext = path.extname(originalName) || "";
    const storagePath = path.join(this.baseDir, `${fileId}${ext}`);
    const now = new Date();

    const entry: UploadedFile = {
      fileId,
      originalName,
      mimeType,
      sizeBytes: 0, // Updated after streaming completes
      uploadedAt: now,
      expiresAt: new Date(now.getTime() + FILE_TTL_MS),
      storagePath,
    };

    this.files.set(fileId, entry);
    return { fileId, storagePath };
  }

  /** Updates the file size (called after streaming completes) */
  updateSize(fileId: string, sizeBytes: number): void {
    const entry = this.files.get(fileId);
    if (entry) {
      entry.sizeBytes = sizeBytes;
    }
  }

  /** Retrieves metadata by file ID (null if the TTL has expired) */
  get(fileId: string): UploadedFile | null {
    const entry = this.files.get(fileId);
    if (!entry) return null;
    if (entry.expiresAt < new Date()) {
      // TTL expired -> delete immediately
      void this.deleteEntry(fileId);
      return null;
    }
    return entry;
  }

  /** Cancels the registration and deletes the file (on upload failure) */
  async rollback(fileId: string): Promise<void> {
    await this.deleteEntry(fileId);
  }

  /** Releases all resources, including the temporary directory */
  async destroy(): Promise<void> {
    if (this.cleanupTimer) {
      clearInterval(this.cleanupTimer);
      this.cleanupTimer = null;
    }
    this.files.clear();
    try {
      await fs.rm(this.baseDir, { recursive: true, force: true });
    } catch {
      // Ignore deletion failures
    }
    logger.info("UploadStore destroyed");
  }

  /** Validates whether the MIME type is in the allowlist */
  static isAllowedMimeType(mimeType: string): boolean {
    return ALLOWED_MIME_TYPES.has(mimeType);
  }

  /** Returns the path of the temporary directory */
  getBaseDir(): string {
    return this.baseDir;
  }

  /** Returns the current number of registered files */
  get size(): number {
    return this.files.size;
  }

  /** Cleans up entries whose TTL has expired */
  private async cleanup(): Promise<void> {
    const now = new Date();
    const expired: string[] = [];

    for (const [id, entry] of this.files) {
      if (entry.expiresAt < now) {
        expired.push(id);
      }
    }

    for (const id of expired) {
      await this.deleteEntry(id);
    }

    if (expired.length > 0) {
      logger.info({ count: expired.length }, "Cleaned up expired files");
    }
  }

  /** Removes the entry and deletes its file */
  private async deleteEntry(fileId: string): Promise<void> {
    const entry = this.files.get(fileId);
    if (!entry) return;
    this.files.delete(fileId);
    try {
      await fs.unlink(entry.storagePath);
    } catch {
      // Ignore if the file no longer exists
    }
  }
}
