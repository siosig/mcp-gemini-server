import { z } from "zod";
import { uploadFile, listFiles, getFileStatus, deleteFile } from "../services/gemini_files.js";

export const manageFilesSchema = z.object({
  action: z.enum(["upload", "list", "status", "delete"]),
  file_path: z.string().optional(),
  file_name: z.string().optional(),
  display_name: z.string().optional(),
}).strict();

export type ManageFilesArgs = z.infer<typeof manageFilesSchema>;

export async function handleManageFiles(args: ManageFilesArgs): Promise<string> {
  const { action } = args;

  if (action === "upload") {
    if (!args.file_path) throw new Error("file_path is required for upload action");
    const result = await uploadFile(args.file_path, args.display_name);
    return JSON.stringify(result);
  }

  if (action === "list") {
    const result = await listFiles();
    return JSON.stringify(result, null, 2);
  }

  if (action === "status") {
    if (!args.file_name) throw new Error("file_name is required for status action");
    const result = await getFileStatus(args.file_name);
    return JSON.stringify(result);
  }

  if (action === "delete") {
    if (!args.file_name) throw new Error("file_name is required for delete action");
    await deleteFile(args.file_name);
    return JSON.stringify({ success: true, message: `File ${args.file_name} deleted successfully.` });
  }

  throw new Error(`Invalid action: ${action}`);
}
