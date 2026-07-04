// File-type detection, constants, formatters shared across the file manager.

export const FILE_TYPES = {
    code: ['js', 'jsx', 'ts', 'tsx', 'py', 'rb', 'php', 'java', 'c', 'cpp', 'h', 'go', 'rs', 'swift', 'kt'],
    image: ['jpg', 'jpeg', 'png', 'gif', 'bmp', 'svg', 'webp', 'ico', 'avif'],
    video: ['mp4', 'avi', 'mkv', 'mov', 'wmv', 'webm', 'flv'],
    audio: ['mp3', 'wav', 'flac', 'aac', 'ogg', 'wma', 'm4a'],
    archive: ['zip', 'tar', 'gz', 'rar', '7z', 'bz2', 'xz'],
    data: ['json', 'xml', 'yaml', 'yml', 'csv', 'db', 'sqlite', 'sql', 'toml'],
    text: ['txt', 'md', 'log', 'ini', 'conf', 'cfg', 'env'],
    terminal: ['sh', 'bash', 'zsh', 'fish'],
};

export function getFileType(entry) {
    if (entry.is_dir) return 'folder';
    const ext = (entry.name.split('.').pop() || '').toLowerCase();
    for (const [type, exts] of Object.entries(FILE_TYPES)) {
        if (exts.includes(ext)) return type;
    }
    return 'default';
}

export function getFileExt(entry) {
    if (entry.is_dir) return '';
    const parts = entry.name.split('.');
    if (parts.length < 2 || parts[0] === '') return '';
    return parts.pop().toUpperCase().slice(0, 4);
}

// Re-export the canonical byte formatter so all importers share one impl.
export { formatBytes } from '@/utils/formatBytes';

// Folders that anchor the left-sidebar tree. Backend ALLOWED_ROOTS are
// /home, /var/www, /opt, /srv, /var/log, plus the SERVERKIT_DIR.
export const TREE_ROOTS = [
    { path: '/home', name: 'home' },
    { path: '/var/www', name: 'var/www' },
    { path: '/opt', name: 'opt' },
    { path: '/srv', name: 'srv' },
    { path: '/var/log', name: 'var/log' },
];

export const DEFAULT_PINNED = [
    { path: '/home', name: 'Home' },
    { path: '/var/www', name: 'Web Root' },
    { path: '/var/log', name: 'Logs' },
    { path: '/opt', name: 'Opt' },
    { path: '/srv', name: 'Srv' },
];

// Filter chip definitions are kept in the page since they import lucide icons.
