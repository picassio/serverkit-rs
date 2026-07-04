import { useTheme } from '../../contexts/ThemeContext';
import useDashboardLayout from '../../hooks/useDashboardLayout';
import { ChevronDown, ChevronUp, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';

const ACCENT_PRESETS = [
    { label: 'Indigo', color: '#6366f1' },
    { label: 'Ocean', color: '#0ea5e9' },
    { label: 'Forest', color: '#10b981' },
    { label: 'Sunset', color: '#f97316' },
    { label: 'Rose', color: '#f43f5e' },
    { label: 'Violet', color: '#8b5cf6' },
    { label: 'Amber', color: '#f59e0b' },
    { label: 'Cyan', color: '#06b6d4' },
];

const AppearanceTab = () => {
    const { theme, setTheme, accentColor, setAccentColor } = useTheme();
    const { widgets, toggleWidget, moveWidget, resetLayout } = useDashboardLayout();

    return (
        <div className="settings-section">
            <div className="section-header">
                <h2>Appearance</h2>
                <p>Customize the look and feel of your dashboard</p>
            </div>

            <div className="settings-card">
                <h3>Theme</h3>
                <p>Select your preferred color scheme</p>
                <div className="theme-options">
                    <button type="button"
                        className={`theme-option ${theme === 'dark' ? 'active' : ''}`}
                        onClick={() => setTheme('dark')}
                    >
                        <div className="theme-preview dark">
                            <div className="preview-sidebar"></div>
                            <div className="preview-content">
                                <div className="preview-card"></div>
                                <div className="preview-card"></div>
                            </div>
                        </div>
                        <span>Dark</span>
                    </button>
                    <button type="button"
                        className={`theme-option ${theme === 'light' ? 'active' : ''}`}
                        onClick={() => setTheme('light')}
                    >
                        <div className="theme-preview light">
                            <div className="preview-sidebar"></div>
                            <div className="preview-content">
                                <div className="preview-card"></div>
                                <div className="preview-card"></div>
                            </div>
                        </div>
                        <span>Light</span>
                    </button>
                    <button type="button"
                        className={`theme-option ${theme === 'system' ? 'active' : ''}`}
                        onClick={() => setTheme('system')}
                    >
                        <div className="theme-preview system">
                            <div className="preview-sidebar"></div>
                            <div className="preview-content">
                                <div className="preview-card"></div>
                                <div className="preview-card"></div>
                            </div>
                        </div>
                        <span>System</span>
                    </button>
                </div>
            </div>

            <div className="settings-card">
                <h3>Accent Color</h3>
                <p>Choose the primary accent color used across the interface</p>
                <div className="accent-presets">
                    {ACCENT_PRESETS.map(({ label, color }) => (
                        <button type="button"
                            key={color}
                            className={`accent-preset${accentColor === color ? ' active' : ''}`}
                            onClick={() => setAccentColor(color)}
                        >
                            <span className="accent-swatch" style={{ background: color }} />
                            <span className="accent-label">{label}</span>
                        </button>
                    ))}
                </div>
                <div className="accent-custom">
                    <label className="accent-custom-label">Custom color</label>
                    <div className="accent-custom-row">
                        <input
                            type="color"
                            className="accent-custom-input"
                            value={accentColor}
                            onChange={(e) => setAccentColor(e.target.value)}
                        />
                        <span className="accent-custom-hex">{accentColor.toUpperCase()}</span>
                    </div>
                </div>
            </div>

            <div className="settings-card">
                <h3>Dashboard Widgets</h3>
                <p>Toggle visibility and reorder widgets on the dashboard</p>
                <div className="widget-list">
                    {widgets.map((widget, idx) => (
                        <div key={widget.id} className={`widget-item${!widget.visible ? ' widget-item--hidden' : ''}`}>
                            <div className="widget-item__info">
                                <Switch
                                    checked={widget.visible}
                                    onCheckedChange={() => toggleWidget(widget.id)}
                                />
                                <span className="widget-item__label">{widget.label}</span>
                            </div>
                            <div className="widget-item__controls">
                                <button type="button"
                                    className="widget-move-btn"
                                    onClick={() => moveWidget(widget.id, 'up')}
                                    disabled={idx === 0}
                                    title="Move up"
                                >
                                    <ChevronUp size={14} />
                                </button>
                                <button type="button"
                                    className="widget-move-btn"
                                    onClick={() => moveWidget(widget.id, 'down')}
                                    disabled={idx === widgets.length - 1}
                                    title="Move down"
                                >
                                    <ChevronDown size={14} />
                                </button>
                            </div>
                        </div>
                    ))}
                </div>
                <Button variant="outline" size="sm" onClick={resetLayout} style={{ marginTop: '12px' }}>
                    <RotateCcw size={14} />
                    Reset to defaults
                </Button>
            </div>

        </div>
    );
};

export default AppearanceTab;
