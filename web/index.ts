const diskSize = 1 << 20;
const processorRate = 1024 * 1024 * 1.5;
// active disk light remains lit for 0.5 s after disk became idle
const diskActiveLightTime = processorRate / 2;

class Disk {
    public label: string;
    public data: Uint8Array;

    constructor(label: string, data?: Uint8Array) {
        this.label = label;
        this.data = data ? data : new Uint8Array(diskSize);
    }

    copy() {
        const disk = new Disk(this.label);
        for (let i = 0; i < diskSize; i++) {
            disk.data[i] = this.data[i];
        }
        return disk;
    }

    isSame(other: Disk): boolean {
        const a = this.data;
        const b = other.data;
        for (let i = 0; i < diskSize; i++) {
            if (a[i] !== b[i]) {
                return false;
            }
        }
        return true;
    }
}

class InsertedDisk {
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
    private wasm: WebAssembly.Instance;
    private keys: {[key: string]: boolean};
    private timeBudget: number;
    public slots: (InsertedDisk | null)[];

    constructor(wasm: WebAssembly.Instance) {
        this.wasm = wasm;
        this.keys = {};
        this.slots = [null, null];
        this.timeBudget = 0;
        (this.wasm.exports.initialize as any)();
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
            const slot = this.slots[disk];
            if (slot !== null) {
                const diskStatus = (this.wasm.exports.disk_stats as any)(disk);
                const modified = ((diskStatus >> 1) & 1) !== 0;
                const idleTime = diskStatus >> 2;
                slot.modified = modified;
                slot.working = idleTime <= diskActiveLightTime;
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

    public screenSize(): { width: number, height: number } {
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
        const inserted = new InsertedDisk(disk);
        const buffer = this.diskBuffer(index);
        buffer.set(disk.data);
        this.slots[index] = inserted;
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

    public logMessage(ptr: number, len: number) {
        const buffer = (this.wasm.exports.memory as any).buffer;
        const messageBuffer = new Uint8Array(buffer, ptr, len);
        var message = new TextDecoder("utf-8").decode(messageBuffer);
        console.log(`emulator log: ${message}`);
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

let logCallback: ((ptr: number, len: number) => void) | undefined;

const imports = {
    env: {
        log_message: (ptr: number, len: number) => {
            if (logCallback) {
                logCallback(ptr, len);
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
    logCallback = (ptr, len) => emulator.logMessage(ptr, len);

    const emulatorScreenSize = emulator.screenSize();
    const renderer = new Renderer(emulatorScreenSize);

    const db = new LocalDb();

    (window as any).emulator = emulator;

    const app = new App(renderer, emulator, db);

    document.getElementById('reset')!.onclick = () => app.clickedReset();

    window.addEventListener('keydown', e => {
        if (e.which >= 37 && e.which <= 40) {
            e.preventDefault();
        }
        app.keyDown(e.which);
    });
    window.addEventListener('keyup', e => {
        if (e.which >= 37 && e.which <= 40) {
            e.preventDefault();
        }
        app.keyUp(e.which);
    });
    window.addEventListener('blur', _ => {
        app.blur();
    });

    let lastTime = performance.now();

    function frame() {
        const currentTime = performance.now();
        const delta = Math.max(0, (currentTime - lastTime) / 1000);
        lastTime = currentTime;
        app.frame(delta);
        window.requestAnimationFrame(frame);
    }

    window.requestAnimationFrame(frame);
}

type LibraryDiskState = 'ok' | 'saving' | 'deleting' | 'failed';

class LibraryDisk {
    public disk: Disk;
    public state: LibraryDiskState;

    constructor(disk: Disk, state: LibraryDiskState) {
        this.disk = disk;
        this.state = state;
    }

    public canInteract(): boolean {
        return this.state === 'ok' || this.state === 'failed';
    }
}

type LocatedDisk = { disk: Disk, place: 'library' | 'slot' };

interface DiskDb {
    deleteDisk(disk: Disk): Promise<void>;
    saveDisk(disk: Disk): Promise<void>;
    getDisks(): Promise<Disk[]>;
}

class NullDb implements DiskDb {
    public deleteDisk(disk: Disk): Promise<void> {
        return new Promise((resolve, _) => resolve());
    }

    public saveDisk(disk: Disk): Promise<void> {
        return new Promise((resolve, _) => resolve());
    }

    public getDisks(): Promise<Disk[]> {
        return new Promise((resolve, _) => resolve([]));
    }
}

class OpenedLocalDb implements DiskDb {
    private db: IDBDatabase;

    constructor(db: IDBDatabase) {
        this.db = db;
    }

    public saveDisk(disk: Disk): Promise<void> {
        const request = this.db
            .transaction(['disks'], 'readwrite')
            .objectStore('disks')
            .put(disk);
        return new Promise((resolve, reject) => {
            request.onsuccess = () => resolve();
            request.onerror = (e: any) => reject(e);
        });
    }

    public deleteDisk(disk: Disk): Promise<void> {
        const request = this.db
            .transaction(['disks'], 'readwrite')
            .objectStore('disks')
            .delete(disk.label);
        return new Promise((resolve, reject) => {
            request.onsuccess = () => resolve();
            request.onerror = (e: any) => reject(e);
        });
    }

    public getDisks(): Promise<Disk[]> {
        const request = this.db
            .transaction(['disks'])
            .objectStore('disks')
            .openCursor();
        return new Promise((resolve, reject) => {
            const disks: Disk[] = [];
            request.onsuccess = (e: any) => {
                const cursor = e.target.result;
                if (cursor) {
                    const label = cursor.value.label;
                    const data = cursor.value.data;
                    disks.push(new Disk(label, data));
                    cursor.continue();
                } else {
                    resolve(disks);
                }
            };
            request.onerror = (e: any) => reject(e);
        });
    }
}

class LocalDb implements DiskDb {
    private db: Promise<OpenedLocalDb>;

    constructor() {
        const request = indexedDB.open('disk-db', 2);
        request.onupgradeneeded = (e: any) => {
            const db = e.target.result;
            db.createObjectStore('disks', { keyPath: 'label' });
        };
        this.db = new Promise((resolve, reject) => {
            request.onsuccess = (e: any) => resolve(new OpenedLocalDb(e.target.result));
            request.onerror = reject;
        });
    }

    public saveDisk(disk: Disk): Promise<void> {
        return this.db.then(db => db.saveDisk(disk));
    }

    public deleteDisk(disk: Disk): Promise<void> {
        return this.db.then(db => db.deleteDisk(disk));
    }

    public getDisks(): Promise<Disk[]> {
        return this.db.then(db => db.getDisks());
    }
}

class App {
    private renderer: Renderer;
    private libraryDisks: LibraryDisk[];
    private emulator: Emulator;
    private dragged: LocatedDisk | null;
    private diskDb: DiskDb;

    constructor(renderer: Renderer, emulator: Emulator, diskDb: DiskDb) {
        this.renderer = renderer;
        this.libraryDisks = [];
        this.emulator = emulator;
        this.dragged = null;
        this.diskDb = diskDb;
        this.diskDb.getDisks()
            .then(disks => this.loadedDisks(disks))
            .catch(e => console.error(e));

        this.renderer.renderInit(this);
        this.renderer.renderLibrary(this, this.libraryDisks);
        this.renderer.renderSlots(this, this.emulator.slots);
    }

    private diskIndex(disk: LocatedDisk): number {
        if (disk.place === 'library') {
            for (let i = 0; i < this.libraryDisks.length; i++) {
                if (this.libraryDisks[i].disk === disk.disk) {
                    return i;
                }
            }
        } else if (disk.place === 'slot') {
            for (let i = 0; i < 2; i++) {
                const slot = this.emulator.slots[i];
                if (slot && slot.disk === disk.disk) {
                    return i;
                }
            }
        }
        throw new Error('dragged disk does not exist');
    }

    private loadedDisks(disks: Disk[]) {
        console.log('event: loaded disks');
        this.libraryDisks = [];
        for (const disk of disks) {
            this.libraryDisks.push(new LibraryDisk(disk, 'ok'));
        }
        this.renderer.renderLibrary(this, this.libraryDisks);
    }

    private savedDisk(disk: Disk) {
        console.log('event: saved disk');
        const index = this.diskIndex({ disk, place: 'library' });
        if (this.libraryDisks[index].canInteract()) {
            throw new Error('invalid save');
        }
        this.libraryDisks[index].state = 'ok';
        this.renderer.renderLibrary(this, this.libraryDisks);
    }

    private deletedDisk(disk: Disk) {
        console.log('event: deleted disk');
        const index = this.diskIndex({ disk, place: 'library' });
        if (this.libraryDisks[index].canInteract()) {
            throw new Error('invalid delete');
        }
        this.libraryDisks.splice(index, 1);
        this.renderer.renderLibrary(this, this.libraryDisks);
    }

    private failedDiskSave(disk: Disk, error: any) {
        console.log('event: failed disk save');
        const index = this.diskIndex({ disk, place: 'library' });
        if (this.libraryDisks[index].canInteract()) {
            throw new Error('invalid delete');
        }
        console.error(error);
        this.libraryDisks[index].state = 'failed';
        this.renderer.renderLibrary(this, this.libraryDisks);
    }

    public importDisk(disk: Disk) {
        const libraryDisk = new LibraryDisk(disk, 'saving');
        this.libraryDisks.push(libraryDisk);
        this.libraryDisks.sort((a, b) => naturalCompare(a.disk.label, b.disk.label));
        this.diskDb.saveDisk(disk)
            .then(() => this.savedDisk(disk))
            .catch(e => this.failedDiskSave(disk, e));
        this.renderer.renderLibrary(this, this.libraryDisks);
        this.renderer.renderSlots(this, this.emulator.slots);
    }

    public renameDisk(disk: LocatedDisk, newLabel: string) {
        console.log('event: rename disk');
        const index = this.diskIndex(disk);
        if (disk.place === 'library') {
            if (!this.libraryDisks[index].canInteract()) {
                return;
            }
            let matching = 0;
            for (let i = 0; i < this.libraryDisks.length; i++) {
                if (i !== index && this.libraryDisks[i].disk.label === newLabel) {
                    matching += 1;
                }
            }
            if (matching > 0) {
                return;
            }
            this.libraryDisks[index].disk.label = newLabel;
            this.renderer.renderLibrary(this, this.libraryDisks);
        } else if (disk.place === 'slot') {
            this.emulator.slots[index]!.disk.label = newLabel;
            this.renderer.renderSlots(this, this.emulator.slots);
        }
    }

    public startedDragging(dragged: LocatedDisk) {
        console.log('event: start dragging');
        if (dragged.place === 'library') {
            const index = this.diskIndex(dragged);
            if (!this.libraryDisks[index].canInteract()) {
                return;
            }
        }
        this.dragged = dragged;
    }

    public droppedOnLibrary() {
        console.log('event: drop on library');
        const dragged = this.dragged;
        this.dragged = null;
        if (!dragged) {
            return;
        }
        const draggedIndex = this.diskIndex(dragged);

        if (dragged.place === 'library') {
            return;
        } else if (dragged.place === 'slot') {
            this.emulator.updateDiskContents(draggedIndex);
            const disk = this.emulator.slots[draggedIndex];
            if (!disk) {
                return;
            }
            for (const libraryDisk of this.libraryDisks) {
                if (disk.disk.label === libraryDisk.disk.label) {
                    if (disk.disk.isSame(libraryDisk.disk)) {
                        this.emulator.removeDisk(draggedIndex);
                        this.renderer.renderSlots(this, this.emulator.slots);
                        return;
                    } else {
                        return;
                    }
                }
            }
            const removedDisk = this.emulator.removeDisk(draggedIndex);
            const libraryDisk = new LibraryDisk(removedDisk, 'saving');
            this.libraryDisks.push(libraryDisk);
            this.libraryDisks.sort((a, b) => naturalCompare(a.disk.label, b.disk.label));
            this.diskDb.saveDisk(removedDisk)
                .then(() => this.savedDisk(removedDisk))
                .catch(e => this.failedDiskSave(removedDisk, e));
            this.renderer.renderLibrary(this, this.libraryDisks);
            this.renderer.renderSlots(this, this.emulator.slots);
        }
    }

    public droppedOnSlot(index: number) {
        console.log('event: drop on slot');
        const dragged = this.dragged;
        this.dragged = null;
        if (!dragged) {
            return;
        }
        const draggedIndex = this.diskIndex(dragged);

        if (this.emulator.slots[index] !== null) {
            return;
        }

        if (dragged.place === 'library') {
            const disk = this.libraryDisks[draggedIndex].disk.copy();
            this.emulator.insertDisk(index, disk);
            this.renderer.renderSlots(this, this.emulator.slots);
        } else if (dragged.place === 'slot') {
            if (index !== draggedIndex) {
                const disk = this.emulator.removeDisk(draggedIndex);
                this.emulator.insertDisk(index, disk);
                this.renderer.renderSlots(this, this.emulator.slots);
            }
        }
    }

    public droppedOnTrash() {
        console.log('event: drop on trash');
        const dragged = this.dragged;
        this.dragged = null;
        if (!dragged) {
            return;
        }
        const draggedIndex = this.diskIndex(dragged);

        if (dragged.place === 'library') {
            const disk = this.libraryDisks[draggedIndex];
            disk.state = 'deleting';
            this.diskDb.deleteDisk(disk.disk)
                .then(() => this.deletedDisk(disk.disk))
                .catch(e => this.failedDiskSave(disk.disk, e));
            this.renderer.renderLibrary(this, this.libraryDisks);
        } else if (dragged.place === 'slot') {
            this.emulator.removeDisk(draggedIndex);
            this.renderer.renderSlots(this, this.emulator.slots);
        }
    }

    public addDisk(slot: number, label: string) {
        console.log('event: add disk');
        if (this.emulator.slots[slot] !== null) {
            return;
        }

        this.emulator.insertDisk(slot, new Disk(label));
        this.renderer.renderSlots(this, this.emulator.slots);
    }

    public clickedDownload(disk: LocatedDisk) {
        console.log('event: download disk');
        if (disk.place === 'slot') {
            this.emulator.updateDiskContents(this.diskIndex(disk));
        }
        const filename = disk.disk.label + '.bin';
        const blob = new Blob([disk.disk.data], {type: 'application/octet-stream'});
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
    }

    public clickedReset() {
        this.emulator.reset();
    }

    public keyDown(key: number) {
        this.emulator.keyDown(key);
    }

    public keyUp(key: number) {
        this.emulator.keyUp(key);
    }

    public blur() {
        this.emulator.resetKeys();
    }

    public frame(dt: number) {
        if (dt > 0.2) {
            console.log(`${dt} seconds behind`);
        } else {
            this.emulator.run(dt);
        }
        this.renderer.renderScreen(this.emulator.screen());
        this.renderer.updateInsertedDiskIndicators(this.emulator.slots);
    }
}

class Renderer {
    private slots: HTMLElement[];
    private library: HTMLElement;
    private trash: HTMLElement;
    private canvas: HTMLCanvasElement;
    private canvasCtx: CanvasRenderingContext2D;
    private screenImageData: ImageData;
    private screenPixelCount: number;

    constructor(emulatorScreen: { width: number, height: number }) {
        this.slots = [
            document.getElementById('slot0')!,
            document.getElementById('slot1')!,
        ];
        this.library = document.getElementById('library')!;
        this.trash = document.getElementById('trash')!;
        this.canvas = document.getElementById('screen') as HTMLCanvasElement;
        this.canvas.width = emulatorScreen.width;
        this.canvas.height = emulatorScreen.height;
        this.canvasCtx = this.canvas.getContext('2d')!;
        this.screenImageData = this.canvasCtx.createImageData(emulatorScreen.width, emulatorScreen.height);
        this.screenPixelCount = emulatorScreen.width * emulatorScreen.height;

    }

    public renderInit(app: App) {
        this.trash.ondragover = e => e.preventDefault();
        this.trash.ondrop = e => {
            e.preventDefault();
            app.droppedOnTrash();
        };
        this.library.ondragover = e => e.preventDefault();
        this.library.ondrop = e => {
            e.preventDefault();
            let file: File | null = null;
            if (e.dataTransfer?.items) {
                for (let i = 0; i < e.dataTransfer.items.length; i++) {
                    const item = e.dataTransfer.items[i];
                    if (item.kind === 'file') {
                        file = item.getAsFile()!;
                        break;
                    }
                }
            } else if (e.dataTransfer?.files) {
                for (let i = 0; i < e.dataTransfer.files.length; i++) {
                    file = e.dataTransfer.files[i];
                    break;
                }
            }
            if (file) {
                const label = file.name.endsWith('.bin')
                    ? file.name.substr(0, file.name.length - 4)
                    : file.name;
                const disk = new Disk(label);
                (file.slice(0, diskSize) as any).arrayBuffer().then((buffer: ArrayBuffer) => {
                    const toCopy = Math.min(disk.data.length, buffer.byteLength);
                    new Uint8Array(disk.data.buffer, 0, toCopy).set(new Uint8Array(buffer, 0, toCopy));
                    app.importDisk(disk);
                });
            } else {
                app.droppedOnLibrary();
            }
        };
    }

    public renderLibrary(app: App, disks: LibraryDisk[]) {
        this.library.innerHTML = '';
        for (const disk of disks) {
            const elem = this.createLibraryDiskElement(app, disk);
            this.library.appendChild(elem);
        }
    }

    private createLibraryDiskElement(app: App, disk: LibraryDisk) {
        const d = document.createElement('div');
        if (disk.state === 'deleting' || disk.state === 'saving') {
            d.className = 'disk diskLike inProgressDisk';
        } else if (disk.state === 'failed') {
            d.className = 'disk diskLike failedDisk';
        } else {
            d.className = 'disk diskLike';
        }
        this.fillSlotElement(app, d, { disk: disk.disk, place: 'library' }, disk.canInteract());
        return d;
    }

    private fillSlotElement(app: App, slot: HTMLElement, disk: LocatedDisk, allowInteractions: boolean) {
        slot.ondblclick = null;
        slot.ondragstart = null;
        slot.ondragend = null;
        slot.innerHTML = '';
        slot.draggable = allowInteractions;
        const working = document.createElement('div');
        working.className = 'indicator working';
        slot.appendChild(working);
        const modified = document.createElement('div');
        modified.className = 'indicator modified';
        slot.appendChild(modified);
        const label = document.createElement('span');
        label.className = 'label';
        label.innerText = disk.disk.label;
        if (allowInteractions) {
            label.ondblclick = () => {
                const newLabel = prompt('Enter new name', disk.disk.label);
                if (newLabel != null) {
                    app.renameDisk(disk, newLabel);
                }
            };
        }
        slot.appendChild(label);
        const downloadButton = document.createElement('div');
        downloadButton.className = 'button';
        const downloadSvg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        downloadSvg.setAttribute('width', '13px');
        downloadSvg.setAttribute('height', '13px');
        const polyline1 = document.createElementNS('http://www.w3.org/2000/svg', 'polyline');
        polyline1.setAttribute('points', '6.5,0 6.5,13');
        downloadSvg.appendChild(polyline1);
        const polyline2 = document.createElementNS('http://www.w3.org/2000/svg', 'polyline');
        polyline2.setAttribute('points', '1,7 7,13');
        downloadSvg.appendChild(polyline2);
        const polyline3 = document.createElementNS('http://www.w3.org/2000/svg', 'polyline');
        polyline3.setAttribute('points', '12,7 6,13');
        downloadSvg.appendChild(polyline3);
        downloadButton.appendChild(downloadSvg);
        downloadButton.onclick = () => app.clickedDownload(disk);
        slot.appendChild(downloadButton);
        if (allowInteractions) {
            slot.ondragstart = () => app.startedDragging(disk);
        }
    }

    public renderSlots(app: App, slots: (InsertedDisk | null)[]) {
        for (let i = 0; i < 2; i++) {
            let slot = slots[i];
            this.slots[i].className = 'slot diskLike';
            if (slot === null) {
                this.prepEmptySlot(app, this.slots[i], i);
            } else {
                this.fillSlotElement(app, this.slots[i], { disk: slot.disk, place: 'slot' }, true);
            }
        }
    }

    public updateInsertedDiskIndicators(slots: (InsertedDisk | null)[]) {
        for (let i = 0; i < 2; i++) {
            let slot = slots[i];
            if (slot !== null) {
                const workingIndicator = this.slots[i].firstChild as any;
                const workingClass = slot.working
                    ? 'indicator working active'
                    : 'indicator working';
                if (workingIndicator.className != workingClass) {
                    workingIndicator.className = workingClass;
                }
                const modifiedIndicator = workingIndicator.nextSibling;
                const modifiedClass = slot.modified
                    ? 'indicator modified active'
                    : 'indicator modified';
                if (modifiedIndicator.className != modifiedClass) {
                    modifiedIndicator.className = modifiedClass;
                }
            }
        }
    }

    private prepEmptySlot(app: App, slot: HTMLElement, index: number) {
        slot.ondblclick = null;
        slot.ondragstart = null;
        slot.ondragend = null;
        slot.innerHTML = '';
        slot.draggable = false;
        slot.ondblclick = () => {
            let label = prompt('Enter new disk name');
            if (label !== null) {
                app.addDisk(index, label);
            }
        };
        slot.ondragover = e => e.preventDefault();
        slot.ondrop = e => {
            e.preventDefault();
            app.droppedOnSlot(index);
        };
    }

    public renderScreen(buffer: Uint8Array) {
        const pixels = this.screenImageData.data;
        const pixelCount = this.screenPixelCount;
        for (let i = 0; i < pixelCount; i++) {
            const byte = buffer[i];
            const color = colorPalette[byte];
            pixels[i * 4 + 0] = (color >> 16) & 0xff;
            pixels[i * 4 + 1] = (color >> 8) & 0xff;
            pixels[i * 4 + 2] = (color >> 0) & 0xff;
            pixels[i * 4 + 3] = 255;
        }
        this.canvasCtx.putImageData(this.screenImageData, 0, 0);
    }
}

function naturalCompare(a: string, b: string) {
    function digitCount(s: string, idx: number) {
        const start = idx;
        while (idx < s.length) {
            if (s[idx] < '0' || s[idx] > '9') {
                break;
            }
            idx += 1;
        }
        return idx - start;
    }
    let ia = 0;
    let ib = 0;
    while (ia < a.length && ib < b.length) {
        const ca = a[ia];
        const cb = a[ib];
        if (ca >= '0' && ca <= '9' && cb >= '0' && cb <= '9') {
            const da = digitCount(a, ia);
            const na = parseInt(a.substr(ia, da));
            ia += da;
            const db = digitCount(b, ib);
            const nb = parseInt(b.substr(ib, db));
            ib += db;
            if (na < nb) {
                return -1;
            } else if (na > nb) {
                return 1;
            }
        } else if (ca < cb) {
            return -1;
        } else if (ca > cb) {
            return 1;
        } else {
            ia += 1;
            ib += 1;
        }
    }
    return 0;
}
