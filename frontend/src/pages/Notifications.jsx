import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { Bell, CheckCheck, ScrollText, X } from 'lucide-react';
import api from '../services/api';
import { PageTopbar } from '@/components/ds';
import { Button } from '@/components/ui/button';
import { useAuth } from '../contexts/AuthContext';
import { useNotifications } from '../contexts/NotificationsContext';
import { timeAgo } from '../utils/timeAgo';

const SEVERITY_DOT = {
    critical: '#fb6f6f',
    warning: '#f5b945',
    success: '#3ddc97',
    info: '#6d7cff',
    test: '#9aa1b2',
};

const PAGE_SIZE = 25;

export default function Notifications() {
    const navigate = useNavigate();
    const { isAdmin } = useAuth();
    const { refresh: refreshBell, items: ctxItems = [], dismissNotice } = useNotifications() || {};
    const [items, setItems] = useState([]);
    const [unreadOnly, setUnreadOnly] = useState(false);
    const [unreadCount, setUnreadCount] = useState(0);
    const [loading, setLoading] = useState(true);
    const [hasMore, setHasMore] = useState(false);

    const fetchPage = useCallback(async (startOffset, replace) => {
        setLoading(true);
        try {
            const data = await api.getInbox({ limit: PAGE_SIZE, offset: startOffset, unread: unreadOnly });
            const fresh = data.items || [];
            setItems((prev) => (replace ? fresh : [...prev, ...fresh]));
            setUnreadCount(data.unread_count || 0);
            setHasMore(fresh.length === PAGE_SIZE);
        } catch {
            setHasMore(false);
        } finally {
            setLoading(false);
        }
    }, [unreadOnly]);

    useEffect(() => { fetchPage(0, true); }, [fetchPage]);

    const onItemClick = async (item) => {
        if (item.read) return;
        setItems((prev) => prev.map((it) => (
            it.delivery_id === item.delivery_id ? { ...it, read: true } : it
        )));
        setUnreadCount((c) => Math.max(0, c - 1));
        try { await api.markNotificationRead(item.delivery_id); } catch { /* reconciled on reload */ }
        if (refreshBell) refreshBell();
    };

    const onMarkAll = async () => {
        setItems((prev) => prev.map((it) => ({ ...it, read: true })));
        setUnreadCount(0);
        try { await api.markAllNotificationsRead(); } catch { /* reconciled on reload */ }
        if (refreshBell) refreshBell();
    };

    // Live system notices (admin config hints) come from the shared context so they
    // surface here too, above the bus history.
    const noticeItems = (ctxItems || []).filter((it) => it.kind === 'notice');

    return (
        <>
            <PageTopbar
                icon={<Bell size={18} />}
                title="Notifications"
                meta={unreadCount ? `${unreadCount} unread` : 'All caught up'}
                actions={(
                    <>
                        {isAdmin && (
                            <Button variant="ghost" size="sm" onClick={() => navigate('/admin/notifications')}>
                                <ScrollText size={15} /> Delivery log
                            </Button>
                        )}
                        <Button variant="outline" size="sm" onClick={onMarkAll} disabled={!unreadCount}>
                            <CheckCheck size={15} /> Mark all read
                        </Button>
                    </>
                )}
            />

            <div className="sk-notif-page">
                <div className="sk-notif-page__filters" role="tablist" aria-label="Filter notifications">
                    <button
                        type="button"
                        role="tab"
                        aria-selected={!unreadOnly}
                        className={!unreadOnly ? 'is-active' : ''}
                        onClick={() => setUnreadOnly(false)}
                    >
                        All
                    </button>
                    <button
                        type="button"
                        role="tab"
                        aria-selected={unreadOnly}
                        className={unreadOnly ? 'is-active' : ''}
                        onClick={() => setUnreadOnly(true)}
                    >
                        Unread
                    </button>
                </div>

                {loading && items.length === 0 && noticeItems.length === 0 ? (
                    <div className="sk-notif-page__state">Loading…</div>
                ) : items.length === 0 && noticeItems.length === 0 ? (
                    <div className="sk-notif-page__state">
                        <Bell size={26} aria-hidden="true" />
                        <p>{unreadOnly ? 'No unread notifications.' : 'No notifications yet.'}</p>
                    </div>
                ) : (
                    <ul className="sk-notif-page__list">
                        {noticeItems.map((item) => (
                            <li
                                key={item.delivery_id}
                                className="sk-notif-row is-unread is-notice"
                                onClick={() => item.action_path && navigate(item.action_path)}
                            >
                                <span
                                    className="sk-notif-row__dot"
                                    style={{ background: SEVERITY_DOT[item.severity] || SEVERITY_DOT.info }}
                                    aria-hidden="true"
                                />
                                <div className="sk-notif-row__body">
                                    <div className="sk-notif-row__title">{item.title}</div>
                                    {item.body && <div className="sk-notif-row__text">{item.body}</div>}
                                    {item.action_label && (
                                        <div className="sk-notif-row__action">{item.action_label} →</div>
                                    )}
                                </div>
                                <button
                                    type="button"
                                    className="sk-notif-row__dismiss"
                                    onClick={(e) => { e.stopPropagation(); if (dismissNotice) dismissNotice(item.notice_id); }}
                                    aria-label={`Dismiss ${item.title}`}
                                >
                                    <X size={15} aria-hidden="true" />
                                </button>
                            </li>
                        ))}
                        {items.map((item) => (
                            <li
                                key={item.delivery_id}
                                className={`sk-notif-row${item.read ? '' : ' is-unread'}`}
                                onClick={() => onItemClick(item)}
                            >
                                <span
                                    className="sk-notif-row__dot"
                                    style={{ background: SEVERITY_DOT[item.severity] || SEVERITY_DOT.info }}
                                    aria-hidden="true"
                                />
                                <div className="sk-notif-row__body">
                                    <div className="sk-notif-row__title">{item.title}</div>
                                    {item.body && <div className="sk-notif-row__text">{item.body}</div>}
                                </div>
                                <span className="sk-notif-row__time">{timeAgo(item.created_at)}</span>
                            </li>
                        ))}
                    </ul>
                )}

                {hasMore && !loading && (
                    <div className="sk-notif-page__more">
                        <Button variant="outline" size="sm" onClick={() => fetchPage(items.length, false)}>
                            Load more
                        </Button>
                    </div>
                )}
            </div>
        </>
    );
}
