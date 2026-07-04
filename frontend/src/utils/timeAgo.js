// Back-compat shim: the canonical implementation now lives in ./time.js.
// Existing imports of `utils/timeAgo` keep working; new code should import
// { timeAgo, formatRelativeTime } from 'utils/time'.
export { timeAgo, timeAgo as default } from './time';
