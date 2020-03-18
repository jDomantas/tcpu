const diskSize = 1 << 20;
const processorRate = 1024 * 1024 * 1.5;

class Disk {
    public label: string;
    public readonly: boolean;
    public data: Uint8Array;

    constructor(label: string, readonly: boolean, data?: Uint8Array) {
        this.label = label;
        this.readonly = readonly ? true : false;
        this.data = data ? data : new Uint8Array(diskSize);
    }

    copy() {
        const disk = new Disk(this.label, this.readonly);
        disk.readonly = this.readonly;
        for (let i = 0; i < diskSize; i++) {
            disk.data[i] = this.data[i];
        }
        return disk;
    }
}

class PluggedDisk {
    public disk: Disk;
    public working: boolean;
    public modified: boolean;

    constructor(disk: Disk) {
        this.disk = disk;
        this.working = false;
        this.modified = false;
    }
}

class Emulator {
    public wasm: WebAssembly.Instance;
    public keys: {[key: string]: boolean};
    public slots: (PluggedDisk | null)[];
    public timeBudget: number;

    constructor(wasm: WebAssembly.Instance) {
        this.wasm = wasm;
        this.keys = {};
        this.slots = [null, null];
        this.timeBudget = 0;
    }

    public reset() {
        (this.wasm.exports.reset as any)();
    }

    public run(dt: number) {
        this.timeBudget += dt;
        const cycles = Math.floor(this.timeBudget * processorRate);
        const cycleTime = cycles / processorRate;
        this.timeBudget -= cycleTime;
        (this.wasm.exports.run as any)(cycles);
        for (let disk = 0; disk < 2; disk++) {
            const diskStatus = (this.wasm.exports.disk_stats as any)(disk);
            const modified = ((diskStatus >> 1) & 1) != 0;
            const working = (diskStatus >> 2) <= processorRate / 2;
            const slot = this.slots[disk];
            if (slot !== null) {
                slot.modified = modified;
                slot.working = working;
            }
        }
    }

    public screen() {
        const ptr = (this.wasm.exports.screen_buffer as any)();
        const size = this.screenSize();
        const pixels = size.width * size.height;
        const memory = (this.wasm.exports.memory as any).buffer;
        return new Uint8Array(memory, ptr, pixels);
    }

    public screenSize() {
        return {
            width: (this.wasm.exports.screen_width as any)(),
            height: (this.wasm.exports.screen_height as any)(),
        };
    }

    public keyUp(key: number) {
        if (!this.keys[key]) {
            return;
        }
        this.keys[key] = false;
        (this.wasm.exports.key_up as any)(key);
    }

    public keyDown(key: number) {
        if (this.keys[key]) {
            return;
        }
        this.keys[key] = true;
        (this.wasm.exports.key_down as any)(key);
    }

    public resetKeys() {
        for (const key in this.keys) {
            if (this.keys[key]) {
                this.keys[key] = false;
                (this.wasm.exports.key_up as any)(parseInt(key));
            }
        }
    }

    private diskBuffer(index: number) {
        const ptr = (this.wasm.exports.disk_buffer as any)(index);
        const memory = (this.wasm.exports.memory as any).buffer;
        return new Uint8Array(memory, ptr, diskSize);
    }

    public insertDisk(index: number, disk: Disk) {
        if (index !== 0 && index !== 1) {
            throw new Error(`disk index ${index} is not valid`);
        }
        const plugged = new PluggedDisk(disk);
        const buffer = this.diskBuffer(index);
        buffer.set(disk.data);
        this.slots[index] = plugged;
        (this.wasm.exports.insert_disk as any)(index);
    }

    public removeDisk(index: number): Disk {
        if (index !== 0 && index !== 1) {
            throw new Error(`disk index ${index} is not valid`);
        }
        const slot = this.slots[index];
        if (!slot) {
            throw new Error(`disk #${index} is not present`);
        }
        const disk = slot.disk;
        this.slots[index] = null;
        const buffer = this.diskBuffer(index);
        disk.data.set(buffer);
        return disk;
    }

    public updateDiskContents(index: number) {
        if (index !== 0 && index !== 1) {
            throw new Error(`disk index ${index} is not valid`);
        }
        const slots = this.slots[index];
        if (!slots) {
            throw new Error(`disk #${index} is not present`);
        }
        const buffer = this.diskBuffer(index);
        slots.disk.data.set(buffer);
    }
}

function getColor(r: number, g: number, b: number) {
    function normalize(p: number, max: number) {
        p /= maxColor;
        p = Math.pow(p, 0.5);
        return Math.round(p * max);
    }

    let maxR = (228 * 0.8) | 0;
    let maxG = (200 * 0.8) | 0;
    let maxB = (255 * 0.8) | 0;
    return {
        r: normalize(r, maxR),
        g: normalize(g, maxG),
        b: normalize(b, maxB),
    };
}

const maxColor = 1;
const colorPalette = new Array(256);
for (let i = 0; i < 256; i++) {
    const r = (i >> 2) & 1;
    const g = (i >> 1) & 1;
    const b = (i >> 0) & 1;
    const color = getColor(r, g, b);
    colorPalette[i] = (color.r << 16) | (color.g << 8) | (color.b << 0);
}

function logMessage(emulator: Emulator, ptr: number, len: number) {
    const buffer = (emulator.wasm.exports.memory as any).buffer;
    const messageBuffer = new Uint8Array(buffer, ptr, len);
    var message = new TextDecoder("utf-8").decode(messageBuffer);
    console.log(`emulator log: ${message}`);
}

let logEmulatorHack: Emulator | null;

const imports = {
    env: {
        log_message: (ptr: number, len: number) => {
            if (logEmulatorHack) {
                logMessage(logEmulatorHack, ptr, len);
            } else {
                console.log('cannot log message from emulator');
            }
        },
    },
};

function onLoad() {
    WebAssembly
        .instantiateStreaming(fetch("/tcpu.wasm"), imports)
        .then(main);
}

function main(wasm: WebAssembly.WebAssemblyInstantiatedSource) {
    const emulator = new Emulator(wasm.instance);
    logEmulatorHack = emulator;

    document.getElementById('reset')!.onclick = () => emulator.reset();

    window.addEventListener('keydown', (e) => {
        emulator.keyDown(e.which);
        if (e.which >= 37 && e.which <= 40) {
            e.preventDefault();
        }
    });
    window.addEventListener('keyup', (e) => {
        emulator.keyUp(e.which);
        e.preventDefault();
    });
    window.addEventListener('blur', (e) => {
        emulator.resetKeys();
    });

    const diskManager = new DiskManager(emulator);
    diskManager.disksChanged();

    const canvas = document.getElementById('screen') as any;
    const screenSize = emulator.screenSize();
    canvas.width = screenSize.width;
    canvas.height = screenSize.height;
    const ctx = canvas.getContext('2d');
    const imageData = ctx.createImageData(screenSize.width, screenSize.height);

    let lastTime = performance.now();

    function frame() {
        const currentTime = performance.now();
        const delta = Math.max(0, (currentTime - lastTime) / 1000);
        if (delta > 0.2) {
            console.log(`${delta} seconds behind`);
        } else {
            emulator.run(delta);
        }
        lastTime = currentTime;
        const screenBuffer = emulator.screen();
        const pixels = imageData.data;
        const pixelCount = screenSize.width * screenSize.height;
        for (let i = 0; i < pixelCount; i++) {
            const byte = screenBuffer[i];
            const color = colorPalette[byte];
            pixels[i * 4 + 0] = (color >> 16) & 0xff;
            pixels[i * 4 + 1] = (color >> 8) & 0xff;
            pixels[i * 4 + 2] = (color >> 0) & 0xff;
            pixels[i * 4 + 3] = 255;
        }
        ctx.putImageData(imageData, 0, 0);
        diskManager.updatePluggedDisks();
        
        window.requestAnimationFrame(frame);
    }

    window.requestAnimationFrame(frame);

    (window as any).importDisk = (data: number[]) => {
        const disk = new Disk('imported', false);
        const len = Math.min(diskSize, data.length);
        for (let i = 0; i < len; i++) {
            disk.data[i] = data[i] & 0xff;
        }
        diskManager.libraryDisks.push(disk);
        diskManager.disksChanged();
    };
}

class DbManager {
    constructor() {
        
    }

    saveDisks(disks: Disk[]) {
        
    }
}

class DiskManager {
    private emulator: Emulator;
    private slots: HTMLElement[];
    private library: HTMLElement;
    private trash: HTMLElement;
    public libraryDisks: Disk[];
    private draggedDisk: Disk | null;

    constructor(emulator: Emulator) {
        this.emulator = emulator;
        this.slots = [
            document.getElementById('slot0')!,
            document.getElementById('slot1')!,
        ];
        this.library = document.getElementById('library')!;
        this.trash = document.getElementById('trash')!;
        this.libraryDisks = [];
        this.draggedDisk = null;
        this.library.ondragover = e => {
            e.preventDefault();
        };
        this.library.ondrop = e => {
            e.preventDefault();
            if (!this.draggedDisk) {
                return;
            }
            let draggedDiskIndex = null;
            for (let i = 0; i < this.emulator.slots.length; i++) {
                const pluggedDisk = this.emulator.slots[i];
                if (pluggedDisk && pluggedDisk.disk == this.draggedDisk) {
                    draggedDiskIndex = i;
                    break;
                }
            }

            if (draggedDiskIndex === null) {
                return;
            }

            let addToLibrary = true;

            for (let i = 0; i < this.libraryDisks.length; i++) {
                if (this.libraryDisks[i].label == this.draggedDisk.label) {
                    const libraryData = this.libraryDisks[i].data;
                    const droppedData = this.draggedDisk.data;
                    let isSame = true;
                    for (let j = 0; j < diskSize; j++) {
                        if (libraryData[j] != droppedData[j]) {
                            isSame = false;
                            break;
                        }
                    }
                    if (isSame) {
                        addToLibrary = false;
                        break;
                    } else {
                        return;
                    }
                }
            }

            this.emulator.removeDisk(draggedDiskIndex);
            if (addToLibrary) {
                this.libraryDisks.push(this.draggedDisk);
            }
            this.draggedDisk = null;
            this.disksChanged();
        };
        this.trash.ondragover = e => {
            e.preventDefault();
        };
        this.trash.ondrop = e => {
            e.preventDefault();
            for (let i = 0; i < this.emulator.slots.length; i++) {
                const pluggedDisk = this.emulator.slots[i];
                if (!pluggedDisk) {
                    continue;
                }
                const disk = pluggedDisk.disk;
                if (disk == this.draggedDisk) {
                    this.draggedDisk = null;
                    this.emulator.removeDisk(i);
                    this.disksChanged();
                    return;
                }
            }
            for (let i = 0; i < this.libraryDisks.length; i++) {
                if (this.libraryDisks[i] == this.draggedDisk) {
                    this.draggedDisk = null;
                    if (this.libraryDisks[i].readonly) {
                        return;
                    }
                    this.libraryDisks.splice(i, 1);
                    this.disksChanged();
                    break;
                }
            }
        };
    }

    createDiskElement(disk: Disk) {
        const d = document.createElement('div');
        d.className = 'disk diskLike';
        this.fillSlotElement(d, disk, null);
        return d;
    }

    fillSlotElement(slot: HTMLElement, disk: Disk, pluggedIndex: number | null) {
        slot.ondblclick = null;
        slot.ondragstart = null;
        slot.ondragend = null;
        slot.innerHTML = '';
        slot.draggable = true;
        const working = document.createElement('div');
        working.className = 'indicator working';
        slot.appendChild(working);
        const modified = document.createElement('div');
        modified.className = 'indicator modified';
        slot.appendChild(modified);
        const label = document.createElement('span');
        label.className = 'label';
        label.innerText = disk.label;
        label.ondblclick = () => {
            const newLabel = prompt('Enter new name', disk.label);
            if (newLabel != null) {
                disk.label = newLabel;
                this.disksChanged();
            }
        };
        slot.appendChild(label);
        const readonlyButton = document.createElement('div');
        readonlyButton.className = disk.readonly ? 'button active' : 'button';
        const buttonText = document.createElement('span');
        buttonText.innerText = 'R';
        buttonText.className = 'btntext';
        readonlyButton.appendChild(buttonText);
        readonlyButton.onclick = () => {
            disk.readonly = !disk.readonly;
            this.disksChanged();
        };
        slot.appendChild(readonlyButton);
        const downloadButton = document.createElement('div');
        downloadButton.className = 'button';
        const downloadSvg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        downloadSvg.setAttribute('width', '20px');
        downloadSvg.setAttribute('height', '20px');
        const downloadPolyline = document.createElementNS('http://www.w3.org/2000/svg', 'polyline');
        downloadPolyline.setAttribute('points', '7.5,4.5 12.5,4.5 12.5,10.5 14.5,10.5 14.5,11.5 10.5,15.5 9.5,15.5 5.5,11.5 5.5,10.5 7.5,10.5 7.5,4.5');
        downloadSvg.appendChild(downloadPolyline);
        downloadButton.appendChild(downloadSvg);
        downloadButton.onclick = () => {
            if (pluggedIndex !== null) {
                this.emulator.updateDiskContents(pluggedIndex);
            }
            const filename = disk.label + '.bin';
            const blob = new Blob([disk.data], {type: 'application/octet-stream'});
            if (window.navigator.msSaveOrOpenBlob as unknown) {
                window.navigator.msSaveBlob(blob, filename);
            } else {
                const elem = window.document.createElement('a');
                elem.href = window.URL.createObjectURL(blob);
                elem.download = filename;
                document.body.appendChild(elem);
                elem.click();        
                document.body.removeChild(elem);
            }
        };
        slot.appendChild(downloadButton);
        slot.ondragstart = () => {
            this.draggedDisk = disk;
        };
        slot.ondragend = () => {
            this.draggedDisk = null;
        };
    }

    prepEmptySlot(slot: HTMLElement, index: number) {
        slot.ondblclick = null;
        slot.ondragstart = null;
        slot.ondragend = null;
        slot.innerHTML = '';
        slot.draggable = false;
        slot.ondblclick = () => {
            let label = prompt('Enter new disk name');
            if (label !== null) {
                const disk = new Disk(label, false);
                this.emulator.insertDisk(index, disk);
                this.disksChanged();
            }
        };
        slot.ondragover = e => {
            e.preventDefault();
        };
        slot.ondrop = e => {
            e.preventDefault();
            for (let i = 0; i < this.libraryDisks.length; i++) {
                const disk = this.libraryDisks[i];
                if (disk == this.draggedDisk) {
                    this.draggedDisk = null;
                    if (disk.readonly) {
                        this.emulator.insertDisk(index, disk.copy());
                    } else {
                        this.libraryDisks.splice(i, 1);
                        this.emulator.insertDisk(index, disk);
                    }
                    this.disksChanged();
                    break;
                }
            }
        };
    }

    disksChanged() {
        this.libraryDisks.sort((a, b) => a.label.localeCompare(b.label));
        this.library.innerHTML = '';
        for (const disk of this.libraryDisks) {
            const elem = this.createDiskElement(disk);
            this.library.appendChild(elem);
        }
        for (let i = 0; i < 2; i++) {
            let plugged = this.emulator.slots[i];
            if (plugged === null) {
                this.slots[i].className = 'slot diskLike';
                this.prepEmptySlot(this.slots[i], i);
            } else {
                this.slots[i].className = 'disk diskLike';
                this.fillSlotElement(this.slots[i], plugged.disk, i);
            }
        }
    }

    updatePluggedDisks() {
        for (let i = 0; i < 2; i++) {
            let plugged = this.emulator.slots[i];
            if (plugged !== null) {
                const workingIndicator = this.slots[i].firstChild as any;
                const workingClass = plugged.working
                    ? 'indicator working active'
                    : 'indicator working';
                if (workingIndicator.className != workingClass) {
                    workingIndicator.className = workingClass;
                }
                const modifiedIndicator = workingIndicator.nextSibling;
                const modifiedClass = plugged.modified
                    ? 'indicator modified active'
                    : 'indicator modified';
                if (modifiedIndicator.className != modifiedClass) {
                    modifiedIndicator.className = modifiedClass;
                }
            }
        }
    }
}
