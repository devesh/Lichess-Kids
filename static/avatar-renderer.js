// SVG Assets and Dynamic Avatar Renderer for Lichess Kids

const SVG_BASES = {
    kid_boy: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <!-- Body -->
            <path d="M 60 180 C 60 150, 140 150, 140 180 Z" fill="#4dabf7" stroke="#1c7ed6" stroke-width="3" />
            <path d="M 90 155 L 90 170 L 110 170 L 110 155 Z" fill="#ffd8a8" stroke="#f76707" stroke-width="2" />
            <!-- Head -->
            <circle cx="100" cy="100" r="50" fill="#ffe066" stroke="#f59f00" stroke-width="3" />
            <!-- Eyes -->
            <circle cx="85" cy="95" r="7" fill="#212529" />
            <circle cx="83" cy="93" r="2.5" fill="#fff" />
            <circle cx="115" cy="95" r="7" fill="#212529" />
            <circle cx="113" cy="93" r="2.5" fill="#fff" />
            <!-- Smile -->
            <path d="M 85 118 Q 100 135 115 118" fill="none" stroke="#e03131" stroke-width="4" stroke-linecap="round" />
            <!-- Cheeks -->
            <circle cx="75" cy="110" r="6" fill="#ffc9c9" opacity="0.6" />
            <circle cx="125" cy="110" r="6" fill="#ffc9c9" opacity="0.6" />
            <!-- Default Hair -->
            <path d="M 50 100 C 45 60, 155 60, 150 100 C 130 90, 70 90, 50 100 Z" fill="#862e9c" />
        </svg>
    `,
    kid_girl: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <!-- Body -->
            <path d="M 60 180 C 60 150, 140 150, 140 180 Z" fill="#ff8787" stroke="#fa5252" stroke-width="3" />
            <path d="M 90 155 L 90 170 L 110 170 L 110 155 Z" fill="#fdd8a8" stroke="#f76707" stroke-width="2" />
            <!-- Head -->
            <circle cx="100" cy="100" r="50" fill="#ffd8a8" stroke="#e67e22" stroke-width="3" />
            <!-- Eyes -->
            <circle cx="85" cy="95" r="7" fill="#212529" />
            <circle cx="83" cy="93" r="2.5" fill="#fff" />
            <circle cx="115" cy="95" r="7" fill="#212529" />
            <circle cx="113" cy="93" r="2.5" fill="#fff" />
            <!-- Smile -->
            <path d="M 85 118 Q 100 135 115 118" fill="none" stroke="#e03131" stroke-width="4" stroke-linecap="round" />
            <!-- Hair Braids -->
            <circle cx="45" cy="115" r="14" fill="#d9480f" />
            <circle cx="155" cy="115" r="14" fill="#d9480f" />
            <path d="M 50 100 C 45 55, 155 55, 150 100 Z" fill="#d9480f" />
        </svg>
    `,
    cat: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <!-- Body -->
            <path d="M 60 180 C 60 145, 140 145, 140 180 Z" fill="#ff922b" stroke="#d9480f" stroke-width="3" />
            <!-- Ears -->
            <polygon points="50,70 80,40 85,85" fill="#ff922b" stroke="#d9480f" stroke-width="3" />
            <polygon points="58,68 78,48 81,78" fill="#ffc078" />
            <polygon points="150,70 120,40 115,85" fill="#ff922b" stroke="#d9480f" stroke-width="3" />
            <polygon points="142,68 122,48 119,78" fill="#ffc078" />
            <!-- Head -->
            <circle cx="100" cy="100" r="50" fill="#ff922b" stroke="#d9480f" stroke-width="3" />
            <!-- Eyes -->
            <ellipse cx="80" cy="95" rx="6" ry="8" fill="#212529" />
            <circle cx="78" cy="92" r="2.5" fill="#fff" />
            <ellipse cx="120" cy="95" rx="6" ry="8" fill="#212529" />
            <circle cx="118" cy="92" r="2.5" fill="#fff" />
            <!-- Snout & Nose -->
            <polygon points="96,108 104,108 100,113" fill="#e03131" />
            <path d="M 94 116 Q 100 122 106 116" fill="none" stroke="#212529" stroke-width="2" />
            <!-- Whiskers -->
            <line x1="72" y1="112" x2="45" y2="108" stroke="#495057" stroke-width="2" />
            <line x1="72" y1="117" x2="42" y2="119" stroke="#495057" stroke-width="2" />
            <line x1="128" y1="112" x2="155" y2="108" stroke="#495057" stroke-width="2" />
            <line x1="128" y1="117" x2="158" y2="119" stroke="#495057" stroke-width="2" />
        </svg>
    `,
    dog: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <!-- Body -->
            <path d="M 60 180 C 60 145, 140 145, 140 180 Z" fill="#a9e34b" stroke="#74b816" stroke-width="3" />
            <!-- Floppy Ears -->
            <path d="M 45 75 C 30 75, 35 125, 55 120 Z" fill="#868e96" stroke="#495057" stroke-width="3" />
            <path d="M 155 75 C 170 75, 165 125, 145 120 Z" fill="#868e96" stroke="#495057" stroke-width="3" />
            <!-- Head -->
            <circle cx="100" cy="100" r="50" fill="#f1f3f5" stroke="#ced4da" stroke-width="3" />
            <!-- Spots -->
            <ellipse cx="80" cy="90" rx="14" ry="18" fill="#868e96" opacity="0.4" />
            <!-- Eyes -->
            <circle cx="80" cy="95" r="7" fill="#212529" />
            <circle cx="78" cy="92" r="2.5" fill="#fff" />
            <circle cx="120" cy="95" r="7" fill="#212529" />
            <circle cx="118" cy="92" r="2.5" fill="#fff" />
            <!-- Nose & Mouth -->
            <ellipse cx="100" cy="112" rx="7" ry="5" fill="#212529" />
            <path d="M 94 118 Q 100 125 106 118" fill="none" stroke="#212529" stroke-width="2.5" stroke-linecap="round" />
        </svg>
    `,
    alien: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <!-- Body -->
            <path d="M 60 180 C 60 145, 140 145, 140 180 Z" fill="#69db7c" stroke="#2b8a3e" stroke-width="3" />
            <!-- Antennae -->
            <line x1="80" y1="58" x2="65" y2="35" stroke="#2b8a3e" stroke-width="4" />
            <circle cx="65" cy="35" r="8" fill="#ffd23f" stroke="#2b8a3e" stroke-width="2" />
            <line x1="120" y1="58" x2="135" y2="35" stroke="#2b8a3e" stroke-width="4" />
            <circle cx="135" cy="35" r="8" fill="#ffd23f" stroke="#2b8a3e" stroke-width="2" />
            <!-- Head -->
            <ellipse cx="100" cy="105" rx="55" ry="45" fill="#69db7c" stroke="#2b8a3e" stroke-width="3" />
            <!-- Three Eyes -->
            <circle cx="75" cy="98" r="8" fill="#fff" stroke="#2b8a3e" stroke-width="1.5" />
            <circle cx="75" cy="98" r="4" fill="#364fc7" />
            <circle cx="100" cy="90" r="10" fill="#fff" stroke="#2b8a3e" stroke-width="1.5" />
            <circle cx="100" cy="90" r="5" fill="#364fc7" />
            <circle cx="125" cy="98" r="8" fill="#fff" stroke="#2b8a3e" stroke-width="1.5" />
            <circle cx="125" cy="98" r="4" fill="#364fc7" />
            <!-- Mouth -->
            <path d="M 85 125 Q 100 138 115 125" fill="none" stroke="#2b8a3e" stroke-width="4" stroke-linecap="round" />
        </svg>
    `
};

const SVG_ITEMS = {
    // Hats
    party_hat: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <polygon points="100,10 70,60 130,60" fill="#ff6b6b" stroke="#e03131" stroke-width="2" />
            <circle cx="100" cy="10" r="6" fill="#ffd23f" />
            <line x1="80" y1="40" x2="120" y2="40" stroke="#fff" stroke-dasharray="3,3" stroke-width="2" />
        </svg>
    `,
    crown: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <polygon points="65,65 75,40 100,55 125,40 135,65" fill="#ffd23f" stroke="#f59f00" stroke-width="2" />
            <rect x="65" y="65" width="70" height="8" fill="#ffd23f" stroke="#f59f00" stroke-width="2" />
            <circle cx="75" cy="40" r="3" fill="#ff1744" />
            <circle cx="100" cy="55" r="3" fill="#00e676" />
            <circle cx="125" cy="40" r="3" fill="#364fc7" />
        </svg>
    `,
    cowboy_hat: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <ellipse cx="100" cy="62" rx="35" ry="18" fill="#85583f" stroke="#5c3826" stroke-width="2" />
            <path d="M 50 68 Q 100 78 150 68 C 145 60, 55 60, 50 68 Z" fill="#a06a42" stroke="#5c3826" stroke-width="2" />
        </svg>
    `,
    pirate_hat: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <path d="M 55 68 Q 100 48 145 68 C 135 60, 65 60, 55 68 Z" fill="#212529" stroke="#000" stroke-width="2" />
            <path d="M 75 62 Q 100 66 125 62 Q 100 78 75 62 Z" fill="#212529" stroke="#000" stroke-width="2" />
            <!-- Skull and Crossbones -->
            <circle cx="100" cy="62" r="3.5" fill="#fff" />
            <line x1="95" y1="58" x2="105" y2="66" stroke="#fff" stroke-width="1.2" />
            <line x1="105" y1="58" x2="95" y2="66" stroke="#fff" stroke-width="1.2" />
        </svg>
    `,

    // Tops
    superhero_cape: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <path d="M 68 140 L 40 195 L 160 195 L 132 140 Z" fill="#ff1744" stroke="#d50000" stroke-width="2" />
        </svg>
    `,
    hoodie: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <path d="M 60 180 C 60 148, 140 148, 140 180 Z" fill="#ff6b6b" stroke="#e03131" stroke-width="2.5" />
            <!-- Draw Strings -->
            <line x1="94" y1="150" x2="94" y2="168" stroke="#fff" stroke-width="2" stroke-linecap="round" />
            <line x1="106" y1="150" x2="106" y2="168" stroke="#fff" stroke-width="2" stroke-linecap="round" />
        </svg>
    `,
    royal_robe: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <path d="M 58 180 C 58 146, 142 146, 142 180 Z" fill="#862e9c" stroke="#6b21a8" stroke-width="2.5" />
            <!-- Golden Trim -->
            <path d="M 85 148 L 100 180 L 115 148 Z" fill="#ffd23f" />
            <!-- White fur collar -->
            <ellipse cx="100" cy="146" rx="20" ry="6" fill="#f8f9fa" />
        </svg>
    `,

    // Bottoms (Visual overlays on lower part of body)
    denim_shorts: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <path d="M 64 175 C 64 168, 136 168, 136 175 L 134 188 L 102 188 L 102 180 L 98 180 L 98 188 L 66 188 Z" fill="#364fc7" stroke="#1b2e88" stroke-width="2" />
        </svg>
    `,
    grass_skirt: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <rect x="62" y="166" width="76" height="4" fill="#a06a42" />
            <path d="M 64 170 L 68 190 M 74 170 L 78 192 M 84 170 L 87 194 M 94 170 L 96 195 M 104 170 L 103 195 M 114 170 L 111 194 M 124 170 L 120 192 M 134 170 L 130 190" stroke="#51cf66" stroke-width="5" stroke-linecap="round" />
        </svg>
    `,

    // Hair Add-ons
    mohawk: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <path d="M 100,25 C 103,45, 97,45, 100,55" fill="none" stroke="#f06595" stroke-width="12" stroke-linecap="round" />
            <path d="M 100,25 L 94,40 M 100,30 L 106,45 M 100,20 L 92,35" stroke="#f285c7" stroke-width="3" />
        </svg>
    `,
    rainbow_hair: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <defs>
                <linearGradient id="rainbow-grad" x1="0%" y1="0%" x2="100%" y2="0%">
                    <stop offset="0%" stop-color="#ff1744" />
                    <stop offset="25%" stop-color="#ffd23f" />
                    <stop offset="50%" stop-color="#00e676" />
                    <stop offset="75%" stop-color="#00e5ff" />
                    <stop offset="100%" stop-color="#b967ff" />
                </linearGradient>
            </defs>
            <path d="M 45 92 C 40 70, 160 70, 155 92 C 160 110, 150 145, 145 155 M 45 92 C 40 110, 50 145, 55 155" fill="none" stroke="url(#rainbow-grad)" stroke-width="8" stroke-linecap="round" />
        </svg>
    `,

    // Accessories Held
    magic_wand: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <!-- Wooden shaft -->
            <line x1="135" y1="170" x2="175" y2="120" stroke="#85583f" stroke-width="4" stroke-linecap="round" />
            <!-- Glowing Star -->
            <polygon points="175,120 178,112 186,110 180,104 182,96 175,100 168,96 170,104 164,110 172,112" fill="#ffd23f" stroke="#f59f00" stroke-width="1.5" />
            <circle cx="175" cy="120" r="15" fill="rgba(255, 210, 63, 0.3)" opacity="0.6" />
        </svg>
    `,
    sword: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <!-- Guard and Handle -->
            <line x1="140" y1="160" x2="160" y2="180" stroke="#f59f00" stroke-width="5" />
            <line x1="135" y1="170" x2="155" y2="150" stroke="#e67e22" stroke-width="8" stroke-linecap="round" />
            <!-- Blade -->
            <polygon points="135,165 178,110 184,116 141,171" fill="#ced4da" stroke="#868e96" stroke-width="1.5" />
            <line x1="138" y1="168" x2="181" y2="113" stroke="#adb5bd" stroke-width="1.5" />
        </svg>
    `,
    balloon: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <!-- Balloon String -->
            <path d="M 145 160 Q 155 170 148 190" fill="none" stroke="#adb5bd" stroke-width="1.5" />
            <!-- Balloon Body -->
            <ellipse cx="145" cy="130" rx="18" ry="24" fill="#ff1744" stroke="#d50000" stroke-width="2" />
            <polygon points="142,154 148,154 145,159" fill="#ff1744" stroke="#d50000" stroke-width="1.5" />
            <!-- Highlight -->
            <ellipse cx="138" cy="122" rx="4" ry="7" fill="#fff" opacity="0.6" transform="rotate(-15, 138, 122)" />
        </svg>
    `,

    // Backgrounds (Z-index 1, fallback gradient if equipped)
    space: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <defs>
                <linearGradient id="space-grad" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stop-color="#0b091a" />
                    <stop offset="100%" stop-color="#1b1542" />
                </linearGradient>
            </defs>
            <rect x="0" y="0" width="200" height="200" fill="url(#space-grad)" />
            <!-- Stars -->
            <circle cx="30" cy="40" r="1.5" fill="#fff" opacity="0.8" />
            <circle cx="170" cy="50" r="1" fill="#fff" opacity="0.6" />
            <circle cx="80" cy="160" r="1.5" fill="#fff" opacity="0.9" />
            <circle cx="160" cy="150" r="2" fill="#ffd23f" opacity="0.7" />
            <!-- Planet -->
            <circle cx="40" cy="140" r="12" fill="#ff7675" />
            <path d="M 22 145 Q 40 135 58 145" fill="none" stroke="#ffd23f" stroke-width="2" />
        </svg>
    `,
    forest: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <defs>
                <linearGradient id="forest-grad" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stop-color="#2b8a3e" />
                    <stop offset="100%" stop-color="#0b3a1a" />
                </linearGradient>
            </defs>
            <rect x="0" y="0" width="200" height="200" fill="url(#forest-grad)" />
            <!-- Simple Pine Trees in Background -->
            <polygon points="30,120 15,160 45,160" fill="#082c14" />
            <polygon points="30,100 20,130 40,130" fill="#082c14" />
            <polygon points="170,110 155,150 185,150" fill="#082c14" />
            <polygon points="170,90 160,120 180,120" fill="#082c14" />
        </svg>
    `,
    castle: `
        <svg viewBox="0 0 200 200" width="100%" height="100%">
            <defs>
                <linearGradient id="castle-grad" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" stop-color="#f06595" />
                    <stop offset="100%" stop-color="#4c0519" />
                </linearGradient>
            </defs>
            <rect x="0" y="0" width="200" height="200" fill="url(#castle-grad)" />
            <!-- Castle Silhouette -->
            <rect x="60" y="120" width="80" height="80" fill="#2d0611" />
            <rect x="40" y="100" width="25" height="100" fill="#1f030a" />
            <polygon points="40,100 52.5,70 65,100" fill="#ff8787" />
            <rect x="135" y="100" width="25" height="100" fill="#1f030a" />
            <polygon points="135,100 147.5,70 160,100" fill="#ff8787" />
            <!-- Gateway -->
            <path d="M 85 200 C 85 160, 115 160, 115 200 Z" fill="#f06595" />
        </svg>
    `
};

/**
 * Render layered SVG avatar dynamically
 * @param {HTMLElement} targetElement - Div where the avatar will be loaded
 * @param {string} avatarBase - The base character type (kid_boy, kid_girl, cat, dog, alien)
 * @param {object} equipped - The items equipped under categories: { top, bottom, hat, hair, accessory, background }
 */
export function renderAvatar(targetElement, avatarBase, equipped = {}) {
    if (!targetElement) return;
    targetElement.innerHTML = '';
    
    // Create base container
    const container = document.createElement('div');
    container.style.position = 'relative';
    container.style.width = '100%';
    container.style.height = '100%';
    
    const layers = [
        { type: 'background', content: SVG_ITEMS[equipped.background] || '', zIndex: 1 },
        { type: 'accessory-back', content: (equipped.top === 'superhero_cape') ? SVG_ITEMS.superhero_cape : '', zIndex: 2 },
        { type: 'base', content: SVG_BASES[avatarBase] || SVG_BASES.kid_boy, zIndex: 3 },
        { type: 'hair', content: SVG_ITEMS[equipped.hair] || '', zIndex: 4 },
        { type: 'top', content: (equipped.top && equipped.top !== 'superhero_cape') ? SVG_ITEMS[equipped.top] : '', zIndex: 5 },
        { type: 'bottom', content: SVG_ITEMS[equipped.bottom] || '', zIndex: 6 },
        { type: 'hat', content: SVG_ITEMS[equipped.hat] || '', zIndex: 7 },
        { type: 'accessory-front', content: SVG_ITEMS[equipped.accessory] || '', zIndex: 8 },
    ];
    
    layers.forEach(layer => {
        if (!layer.content) return;
        
        const layerDiv = document.createElement('div');
        layerDiv.style.position = 'absolute';
        layerDiv.style.top = '0';
        layerDiv.style.left = '0';
        layerDiv.style.width = '100%';
        layerDiv.style.height = '100%';
        layerDiv.style.zIndex = layer.zIndex;
        layerDiv.innerHTML = layer.content;
        
        container.appendChild(layerDiv);
    });
    
    targetElement.appendChild(container);
}
