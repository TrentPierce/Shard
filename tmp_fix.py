import re

with open('web/src/components/NetworkStatus.tsx', 'r', encoding='utf-8') as f:
    c = f.read()

c = re.sub(
    r'<div className="stat-row">\s*<span className="stat-label">Rewards</span>.*?</div>\s*</section>',
    '''<div className="text-sm text-muted italic mt-2">
                    Proof-of-Compute ledger and reward distributions are currently on the testnet.
                    <span className="ml-2 px-2 py-0.5 bg-secondary/20 text-secondary text-[10px] uppercase rounded-full">Coming Soon</span>
                </div>
            </section>''',
    c, flags=re.DOTALL
)

with open('web/src/components/NetworkStatus.tsx', 'w', encoding='utf-8') as f:
    f.write(c)
