// Dynamic Avatar Renderer for Lichess Kids
// Loads SVGs and metadata dynamically from the server catalog

export const SVG_BASES = {};
export const SVG_ITEMS = {};

/**
 * Render layered SVG avatar dynamically
 * @param {HTMLElement} targetElement - Div where the avatar will be loaded
 * @param {string} avatarBase - The base character type (cat, dog, panda, fox, alien, robot, etc.)
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
        { type: 'base', content: SVG_BASES[avatarBase] || SVG_BASES.cat || '', zIndex: 3 },
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

/**
 * Dynamically initialize bases and items from loaded catalog
 * @param {object} catalog - The catalog object returned by /api/assets/catalog
 */
export function initAssets(catalog) {
    if (catalog.bases) {
        catalog.bases.forEach(b => {
            SVG_BASES[b.id] = b.svg;
        });
    }
    if (catalog.items) {
        catalog.items.forEach(i => {
            SVG_ITEMS[i.id] = i.svg;
        });
    }
}
