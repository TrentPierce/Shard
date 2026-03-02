export interface ShardConfig {
    nonce?: string;
    model?: string;
}

export class ShardScout {
    private nonce: string | undefined;

    constructor(config: ShardConfig = {}) {
        this.nonce = config.nonce;
        this.injectStyles();
        this.initWorker();
    }

    private injectStyles() {
        const style = document.createElement('style');
        if (this.nonce) {
            style.setAttribute('nonce', this.nonce);
        }
        style.textContent = `
            .shard-scout-container {
                position: fixed;
                bottom: 20px;
                right: 20px;
                background: rgba(15, 23, 42, 0.9);
                color: #38bdf8;
                padding: 12px 16px;
                border-radius: 8px;
                font-family: system-ui, -apple-system, sans-serif;
                font-size: 14px;
                box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
                border: 1px solid rgba(56, 189, 248, 0.2);
                z-index: 9999;
                backdrop-filter: blur(4px);
                transition: opacity 0.3s ease;
            }
        `;
        document.head.appendChild(style);

        const container = document.createElement('div');
        container.className = 'shard-scout-container';
        container.innerHTML = `<span>Shard Scout Active ⚡</span>`;
        document.body.appendChild(container);
    }

    private initWorker() {
        // Example: If the widget dynamically loads external scripts or Web Workers
        // it applies the nonce to comply with tight CSPs.

        const workerScript = `
            self.onmessage = function(e) {
                // Background compute (e.g. PoW or inference)
            };
        `;

        const blob = new Blob([workerScript], { type: 'application/javascript' });
        const workerUrl = URL.createObjectURL(blob);

        const worker = new Worker(workerUrl);
        worker.postMessage({ type: 'INIT_SCOUT' });

        // If we needed to inject further scripts:
        // const script = document.createElement('script');
        // if (this.nonce) script.setAttribute('nonce', this.nonce);
        // script.src = "https://example.com/some-extra-dependency.js";
        // document.head.appendChild(script);
    }
}

// Attach to window globals for simple drop-in usage
if (typeof window !== 'undefined') {
    (window as any).ShardScout = ShardScout;
}
