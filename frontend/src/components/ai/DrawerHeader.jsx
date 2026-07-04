import { useNavigate } from 'react-router-dom';
import { Settings, X } from 'lucide-react';
import { useAuth } from '../../contexts/AuthContext';
import { useServerkitAI } from '../../contexts/AIContext';
import ModeToggle from './ModeToggle';
import ConversationMenu from './ConversationMenu';

const DrawerHeader = () => {
    const { close } = useServerkitAI();
    const { isAdmin } = useAuth();
    const navigate = useNavigate();

    return (
        <header className="sk-ai-header">
            <div className="sk-ai-header__title">
                <span className="sk-ai-header__name">ServerKit AI</span>
                <span className="sk-ai-header__by">powered by Prompture</span>
            </div>
            <div className="sk-ai-header__actions">
                <ModeToggle />
                <ConversationMenu />
                {isAdmin ? (
                    <button
                        type="button"
                        className="sk-ai-iconbtn"
                        aria-label="AI settings"
                        onClick={() => { close(); navigate('/settings?tab=ai'); }}
                    >
                        <Settings size={16} />
                    </button>
                ) : null}
                <button type="button" className="sk-ai-iconbtn" aria-label="Close assistant" onClick={close}>
                    <X size={16} />
                </button>
            </div>
        </header>
    );
};

export default DrawerHeader;
