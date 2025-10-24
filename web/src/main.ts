/**
 * Main application entry point
 */

import './styles/main.css';
import { initWasm } from './utils/wasm';
import { DemoPage } from './pages/demo';
import { IndexPage } from './pages/index';
import { MicrophonePage } from './pages/microphone';
import { PostamblePage } from './pages/postamble';
import { RecordingDecodePage } from './pages/recording-decode';

/**
 * Application router
 */
interface Route {
    path: string;
    title: string;
    component: () => string | Promise<string>;
}

const routes: Route[] = [
    {
        path: '/',
        title: 'Testaudio - Audio Modem',
        component: () => IndexPage(),
    },
    {
        path: '/demo',
        title: 'Demo - Testaudio',
        component: () => DemoPage(),
    },
    {
        path: '/microphone',
        title: 'Microphone - Testaudio',
        component: () => MicrophonePage(),
    },
    {
        path: '/postamble',
        title: 'Postamble - Testaudio',
        component: () => PostamblePage(),
    },
    {
        path: '/recording-decode',
        title: 'Recording Decode - Testaudio',
        component: () => RecordingDecodePage(),
    },
];

/**
 * Get current route path from hash or pathname
 */
function getRoutePath(): string {
    // Use hash routing for simplicity
    const hash = window.location.hash.slice(1) || '/';
    return hash.startsWith('/') ? hash : '/' + hash;
}

/**
 * Find matching route
 */
function findRoute(path: string): Route | undefined {
    return routes.find(route => {
        // Exact match
        if (route.path === path) return true;
        // Root match
        if (route.path === '/' && (path === '/' || path === '')) return true;
        // Handle trailing slashes
        if ((route.path + '/' === path || path + '/' === route.path) && route.path !== '/') return true;
        return false;
    });
}

/**
 * Navigate to a new route
 */
export function navigate(path: string): void {
    window.location.hash = path === '/' ? '' : path;
    renderCurrentRoute();
}

/**
 * Render current route
 */
async function renderCurrentRoute(): Promise<void> {
    const path = getRoutePath();
    const route = findRoute(path);

    if (!route) {
        const app = document.getElementById('app');
        if (app) {
            app.innerHTML = '<div class="card"><h1>404 - Page Not Found</h1></div>';
        }
        return;
    }

    // Update page title
    document.title = route.title;

    // Render component
    const app = document.getElementById('app');
    if (!app) {
        console.error('App container not found');
        return;
    }

    try {
        const html = await route.component();
        app.innerHTML = html;
    } catch (error) {
        console.error('Error rendering route:', error);
        app.innerHTML = `<div class="card"><div class="status status-error">Error loading page: ${error instanceof Error ? error.message : 'Unknown error'}</div></div>`;
    }
}

/**
 * Handle browser back/forward buttons and hash changes
 */
window.addEventListener('popstate', () => {
    renderCurrentRoute();
});

window.addEventListener('hashchange', () => {
    renderCurrentRoute();
});

/**
 * Initialize application
 */
async function initApp(): Promise<void> {
    try {
        // Initialize WASM module
        await initWasm();

        // Render initial route
        await renderCurrentRoute();
    } catch (error) {
        console.error('Failed to initialize app:', error);
        const app = document.getElementById('app');
        if (app) {
            app.innerHTML = `
                <div class="card">
                    <h1>Failed to Initialize</h1>
                    <div class="status status-error">
                        ${error instanceof Error ? error.message : 'Unknown error'}
                    </div>
                    <p>Please refresh the page to try again.</p>
                </div>
            `;
        }
    }
}

// Start application when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initApp);
} else {
    initApp();
}

// Delegate link clicks to handle navigation
document.addEventListener('click', (e: Event) => {
    const target = e.target as HTMLElement;
    const link = target.closest('a[href^="#"]');
    if (link) {
        const href = link.getAttribute('href');
        if (href) {
            const path = href.slice(1); // Remove #
            e.preventDefault();
            navigate(path || '/');
        }
    }
});

// Export navigate for use in components
export { navigate as goTo };
