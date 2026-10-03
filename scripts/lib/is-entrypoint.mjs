import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

/** Compare canonical filesystem paths so spaces, symlinks, and Windows URLs work. */
export function isEntrypoint(moduleUrl, argvPath = process.argv[1]) {
  if (!argvPath) return false;
  try {
    return fs.realpathSync(argvPath) === fs.realpathSync(fileURLToPath(moduleUrl));
  } catch {
    return false;
  }
}
