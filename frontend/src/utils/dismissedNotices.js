// System notices ("common misconfiguration" hints from GET /system/notices) are
// surfaced in two places — the top banner (SystemNotices) for urgent ones, and the
// notification center (bell + page) for the rest. These helpers keep both surfaces
// in sync: dismissing a notice anywhere persists here and clears it everywhere.

const STORAGE_KEY = 'serverkit_dismissed_notices';

export function getDismissedNotices() {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        return raw ? JSON.parse(raw) : [];
    } catch {
        return [];
    }
}

export function addDismissedNotice(id) {
    const ids = getDismissedNotices();
    if (!ids.includes(id)) {
        ids.push(id);
        try {
            localStorage.setItem(STORAGE_KEY, JSON.stringify(ids));
        } catch {
            // ignore — dismissal is best-effort
        }
    }
    return ids;
}

// Only genuinely urgent notices earn a top banner; everything else lives in the
// notification center. Backend can force one with `urgent: true`; otherwise the
// error/critical levels qualify (the setup-hint notices are warning/info).
export function isUrgentNotice(notice) {
    return notice.urgent === true
        || notice.level === 'error'
        || notice.level === 'critical';
}

// Map a /system/notices entry onto the notification-center item shape so it renders
// alongside Notification Bus deliveries in the bell and the Notifications page.
export function noticeToItem(notice) {
    return {
        delivery_id: `notice:${notice.id}`,
        kind: 'notice',
        notice_id: notice.id,
        severity: notice.level === 'error' ? 'critical' : (notice.level || 'info'),
        title: notice.title,
        body: notice.message,
        action_label: notice.action_label,
        action_path: notice.action_path,
        created_at: null,
        read: false,
    };
}
