import {
    Folder, File, FileCode, FileText, FileImage, FileVideo, FileAudio,
    FileArchive, Database, Terminal,
} from 'lucide-react';
import { getFileType } from './fileTypes';

export default function FileIcon({ entry, size = 20 }) {
    const type = getFileType(entry);
    const cls = `file-icon-svg ${type}`;
    switch (type) {
        case 'folder': return <Folder size={size} className={cls} fill="currentColor" fillOpacity={0.15} />;
        case 'code': return <FileCode size={size} className={cls} />;
        case 'image': return <FileImage size={size} className={cls} />;
        case 'video': return <FileVideo size={size} className={cls} />;
        case 'audio': return <FileAudio size={size} className={cls} />;
        case 'archive': return <FileArchive size={size} className={cls} />;
        case 'data': return <Database size={size} className={cls} />;
        case 'text': return <FileText size={size} className={cls} />;
        case 'terminal': return <Terminal size={size} className={cls} />;
        default: return <File size={size} className={cls} />;
    }
}
