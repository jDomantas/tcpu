"use strict";
const diskSize = 1 << 20;
const processorRate = 1024 * 1024 * 1.5;
// active disk light remains lit for 0.5 s after disk became idle
const diskActiveLightTime = processorRate / 2;
class Disk {
    constructor(label, data, id) {
        this.id = id === undefined ? createUuid() : id;
        this.label = label;
        this.data = data === undefined ? new Uint8Array(diskSize) : data;
    }
    copy() {
        const disk = new Disk(this.label);
        disk.data.set(this.data);
        return disk;
    }
    isSame(other) {
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
    constructor(disk) {
        this.disk = disk;
        this.working = false;
        this.modified = false;
    }
}
class Emulator {
    constructor(wasm) {
        this.wasm = wasm;
        this.keys = {};
        this.slots = [null, null];
        this.timeBudget = 0;
        this.wasm.exports.initialize();
    }
    reset() {
        this.wasm.exports.reset();
    }
    run(dt) {
        this.timeBudget += dt;
        const cycles = Math.floor(this.timeBudget * processorRate);
        const cycleTime = cycles / processorRate;
        this.timeBudget -= cycleTime;
        this.wasm.exports.run(cycles);
        for (let disk = 0; disk < 2; disk++) {
            const slot = this.slots[disk];
            if (slot !== null) {
                const diskStatus = this.wasm.exports.disk_stats(disk);
                const modified = ((diskStatus >> 1) & 1) !== 0;
                const idleTime = diskStatus >> 2;
                slot.modified = modified;
                slot.working = idleTime <= diskActiveLightTime;
            }
        }
    }
    screen() {
        const ptr = this.wasm.exports.screen_buffer();
        const size = this.screenSize();
        const pixels = size.width * size.height;
        const memory = this.wasm.exports.memory.buffer;
        return new Uint8Array(memory, ptr, pixels);
    }
    screenSize() {
        return {
            width: this.wasm.exports.screen_width(),
            height: this.wasm.exports.screen_height(),
        };
    }
    keyUp(key) {
        if (!this.keys[key]) {
            return;
        }
        this.keys[key] = false;
        this.wasm.exports.key_up(key);
    }
    keyDown(key) {
        if (this.keys[key]) {
            return;
        }
        this.keys[key] = true;
        this.wasm.exports.key_down(key);
    }
    resetKeys() {
        for (const key in this.keys) {
            if (this.keys[key]) {
                this.keys[key] = false;
                this.wasm.exports.key_up(parseInt(key));
            }
        }
    }
    diskBuffer(index) {
        const ptr = this.wasm.exports.disk_buffer(index);
        const memory = this.wasm.exports.memory.buffer;
        return new Uint8Array(memory, ptr, diskSize);
    }
    insertDisk(index, disk) {
        if (index !== 0 && index !== 1) {
            throw new Error(`disk index ${index} is not valid`);
        }
        const inserted = new InsertedDisk(disk);
        const buffer = this.diskBuffer(index);
        buffer.set(disk.data);
        this.slots[index] = inserted;
        this.wasm.exports.insert_disk(index);
    }
    removeDisk(index) {
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
        this.wasm.exports.remove_disk(index);
        return disk;
    }
    updateDiskContents(index) {
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
    logMessage(ptr, len) {
        const buffer = this.wasm.exports.memory.buffer;
        const messageBuffer = new Uint8Array(buffer, ptr, len);
        var message = new TextDecoder("utf-8").decode(messageBuffer);
        console.log(`emulator log: ${message}`);
    }
}
function getColor(index) {
    function toRgb(h, s, v) {
        while (h < 0)
            h += 360;
        while (h >= 360)
            h -= 360;
        const c = v * s;
        const x = c * (1 - Math.abs(h / 60 % 2 - 1));
        const m = v - c;
        if (0 <= h && h < 60)
            return { r: (c + m) * 255, g: (x + m) * 255, b: (0 + m) * 255 };
        if (60 <= h && h < 120)
            return { r: (x + m) * 255, g: (c + m) * 255, b: (0 + m) * 255 };
        if (120 <= h && h < 180)
            return { r: (0 + m) * 255, g: (c + m) * 255, b: (x + m) * 255 };
        if (180 <= h && h < 240)
            return { r: (0 + m) * 255, g: (x + m) * 255, b: (c + m) * 255 };
        if (240 <= h && h < 300)
            return { r: (x + m) * 255, g: (0 + m) * 255, b: (c + m) * 255 };
        if (300 <= h && h < 360)
            return { r: (c + m) * 255, g: (0 + m) * 255, b: (x + m) * 255 };
        throw new Error(`h = ${h}`);
    }
    const h = (index >> 4) & 15;
    const s = (index >> 3) & 1;
    const v = index & 7;
    if (h === 15) {
        return toRgb(0, 0, (index % 16) / 15);
    }
    return toRgb(h / 15 * 360, s * 0.5 + 0.4, Math.pow(v / 7, 0.9));
}
const colorPalette = new Array(256);
for (let i = 0; i < 256; i++) {
    const color = getColor(i);
    color.r = Math.round(color.r) & 0xff;
    color.g = Math.round(color.g) & 0xff;
    color.b = Math.round(color.b) & 0xff;
    colorPalette[i] = (color.r << 16) | (color.g << 8) | (color.b << 0);
}
let logCallback;
const imports = {
    env: {
        log_message: (ptr, len) => {
            if (logCallback) {
                logCallback(ptr, len);
            }
            else {
                console.log('cannot log message from emulator');
            }
        },
    },
};
function onLoad() {
    WebAssembly
        .instantiateStreaming(fetch("tcpu.wasm"), imports)
        .then(main);
}
function main(wasm) {
    const emulator = new Emulator(wasm.instance);
    logCallback = (ptr, len) => emulator.logMessage(ptr, len);
    const emulatorScreenSize = emulator.screenSize();
    const renderer = new Renderer(emulatorScreenSize);
    const db = new LocalDb();
    const app = new App(renderer, emulator, db);
    window.app = app;
    document.getElementById('reset').onclick = () => app.clickedReset();
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
class LibraryDisk {
    constructor(disk, state) {
        this.disk = disk;
        this.state = state;
    }
    canInteract() {
        return this.state === 'ok' || this.state === 'failed';
    }
}
class NullDb {
    deleteDisk(disk) {
        return new Promise((resolve, _) => resolve());
    }
    saveDisk(disk) {
        return new Promise((resolve, _) => resolve());
    }
    getDisks() {
        return new Promise((resolve, _) => resolve([]));
    }
}
class OpenedLocalDb {
    constructor(db) {
        this.db = db;
    }
    saveDisk(disk) {
        const request = this.db
            .transaction(['disks'], 'readwrite')
            .objectStore('disks')
            .put(disk);
        return new Promise((resolve, reject) => {
            request.onsuccess = () => resolve();
            request.onerror = (e) => reject(e);
        });
    }
    deleteDisk(disk) {
        const request = this.db
            .transaction(['disks'], 'readwrite')
            .objectStore('disks')
            .delete(disk.id);
        return new Promise((resolve, reject) => {
            request.onsuccess = () => resolve();
            request.onerror = (e) => reject(e);
        });
    }
    getDisks() {
        const request = this.db
            .transaction(['disks'])
            .objectStore('disks')
            .openCursor();
        return new Promise((resolve, reject) => {
            const disks = [];
            request.onsuccess = (e) => {
                const cursor = e.target.result;
                if (cursor) {
                    const id = cursor.value.id;
                    const label = cursor.value.label;
                    const data = cursor.value.data;
                    disks.push(new Disk(label, data, id));
                    cursor.continue();
                }
                else {
                    resolve(disks);
                }
            };
            request.onerror = (e) => reject(e);
        });
    }
}
class LocalDb {
    constructor() {
        const request = indexedDB.open('disk-db', 2);
        request.onupgradeneeded = (e) => {
            const db = e.target.result;
            db.createObjectStore('disks', { keyPath: 'id' });
        };
        this.db = new Promise((resolve, reject) => {
            request.onsuccess = (e) => resolve(new OpenedLocalDb(e.target.result));
            request.onerror = reject;
        });
    }
    saveDisk(disk) {
        return this.db.then(db => db.saveDisk(disk));
    }
    deleteDisk(disk) {
        return this.db.then(db => db.deleteDisk(disk));
    }
    getDisks() {
        return this.db.then(db => db.getDisks());
    }
}
class App {
    constructor(renderer, emulator, diskDb) {
        this.renderer = renderer;
        this.libraryDisks = [];
        this.emulator = emulator;
        this.dragged = null;
        this.diskDb = diskDb;
        this.diskDb.getDisks()
            .then(disks => this.loadedDisks(disks))
            .catch(e => console.error(e));
        this.renderer.renderInit(this);
        this.renderLibrary();
        this.renderer.renderSlots(this, this.emulator.slots);
    }
    diskIndex(disk) {
        if (disk.place === 'library') {
            for (let i = 0; i < this.libraryDisks.length; i++) {
                if (this.libraryDisks[i].disk === disk.disk) {
                    return i;
                }
            }
        }
        else if (disk.place === 'slot') {
            for (let i = 0; i < 2; i++) {
                const slot = this.emulator.slots[i];
                if (slot && slot.disk === disk.disk) {
                    return i;
                }
            }
        }
        throw new Error('dragged disk does not exist');
    }
    renderLibrary() {
        this.libraryDisks.sort((a, b) => {
            const byLabel = naturalCompare(a.disk.label, b.disk.label);
            if (byLabel !== 0) {
                return byLabel;
            }
            return a.disk.id.localeCompare(b.disk.id);
        });
        this.renderer.renderLibrary(this, this.libraryDisks);
    }
    loadedDisks(disks) {
        console.log('event: loaded disks');
        this.libraryDisks = [];
        for (const disk of disks) {
            this.libraryDisks.push(new LibraryDisk(disk, 'ok'));
        }
        this.renderLibrary();
    }
    savedDisk(disk) {
        console.log('event: saved disk');
        const index = this.diskIndex({ disk, place: 'library' });
        if (this.libraryDisks[index].canInteract()) {
            throw new Error('invalid save');
        }
        this.libraryDisks[index].state = 'ok';
        this.renderLibrary();
    }
    deletedDisk(disk) {
        console.log('event: deleted disk');
        const index = this.diskIndex({ disk, place: 'library' });
        if (this.libraryDisks[index].canInteract()) {
            throw new Error('invalid delete');
        }
        this.libraryDisks.splice(index, 1);
        this.renderLibrary();
    }
    failedDiskSave(disk, error) {
        console.log('event: failed disk save');
        const index = this.diskIndex({ disk, place: 'library' });
        if (this.libraryDisks[index].canInteract()) {
            throw new Error('invalid delete');
        }
        console.error(error);
        this.libraryDisks[index].state = 'failed';
        this.renderLibrary();
    }
    importDisk(disk) {
        const libraryDisk = new LibraryDisk(disk, 'saving');
        this.libraryDisks.push(libraryDisk);
        this.diskDb.saveDisk(disk)
            .then(() => this.savedDisk(disk))
            .catch(e => this.failedDiskSave(disk, e));
        this.renderLibrary();
        this.renderer.renderSlots(this, this.emulator.slots);
    }
    renameDisk(disk, newLabel) {
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
            const disk = this.libraryDisks[index];
            disk.state = 'saving';
            const oldLabel = disk.disk.label;
            disk.disk.label = newLabel;
            this.diskDb.saveDisk(disk.disk)
                .then(() => this.savedDisk(disk.disk))
                .catch(e => {
                disk.disk.label = oldLabel;
                this.failedDiskSave(disk.disk, e);
            });
            this.renderLibrary();
        }
        else if (disk.place === 'slot') {
            this.emulator.slots[index].disk.label = newLabel;
            this.renderer.renderSlots(this, this.emulator.slots);
        }
    }
    startedDragging(dragged) {
        console.log('event: start dragging');
        if (dragged.place === 'library') {
            const index = this.diskIndex(dragged);
            if (!this.libraryDisks[index].canInteract()) {
                return;
            }
        }
        this.dragged = dragged;
    }
    droppedOnLibrary() {
        console.log('event: drop on library');
        const dragged = this.dragged;
        this.dragged = null;
        if (!dragged) {
            return;
        }
        const draggedIndex = this.diskIndex(dragged);
        if (dragged.place === 'library') {
            return;
        }
        else if (dragged.place === 'slot') {
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
                    }
                    else {
                        return;
                    }
                }
            }
            const removedDisk = this.emulator.removeDisk(draggedIndex);
            const libraryDisk = new LibraryDisk(removedDisk, 'saving');
            this.libraryDisks.push(libraryDisk);
            this.diskDb.saveDisk(removedDisk)
                .then(() => this.savedDisk(removedDisk))
                .catch(e => this.failedDiskSave(removedDisk, e));
            this.renderLibrary();
            this.renderer.renderSlots(this, this.emulator.slots);
        }
    }
    droppedOnSlot(index) {
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
        }
        else if (dragged.place === 'slot') {
            if (index !== draggedIndex) {
                const disk = this.emulator.removeDisk(draggedIndex);
                this.emulator.insertDisk(index, disk);
                this.renderer.renderSlots(this, this.emulator.slots);
            }
        }
    }
    droppedOnTrash() {
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
            this.renderLibrary();
        }
        else if (dragged.place === 'slot') {
            this.emulator.removeDisk(draggedIndex);
            this.renderer.renderSlots(this, this.emulator.slots);
        }
    }
    createDiskInSlot(slot, label) {
        console.log('event: create disk in slot');
        if (this.emulator.slots[slot] !== null) {
            return;
        }
        this.emulator.insertDisk(slot, new Disk(label));
        this.renderer.renderSlots(this, this.emulator.slots);
    }
    clickedDownload(disk) {
        console.log('event: download disk');
        if (disk.place === 'slot') {
            this.emulator.updateDiskContents(this.diskIndex(disk));
        }
        const filename = disk.disk.label + '.bin';
        const blob = new Blob([disk.disk.data], { type: 'application/octet-stream' });
        if (window.navigator.msSaveOrOpenBlob) {
            window.navigator.msSaveBlob(blob, filename);
        }
        else {
            const elem = window.document.createElement('a');
            elem.href = window.URL.createObjectURL(blob);
            elem.download = filename;
            document.body.appendChild(elem);
            elem.click();
            document.body.removeChild(elem);
        }
    }
    clickedReset() {
        console.log('event: reset emulator');
        this.emulator.reset();
    }
    keyDown(key) {
        this.emulator.keyDown(key);
    }
    keyUp(key) {
        this.emulator.keyUp(key);
    }
    blur() {
        this.emulator.resetKeys();
    }
    frame(dt) {
        if (dt > 0.2) {
            console.log(`${dt} seconds behind`);
        }
        else {
            this.emulator.run(dt);
        }
        this.renderer.renderScreen(this.emulator.screen());
        this.renderer.updateInsertedDiskIndicators(this.emulator.slots);
    }
}
class Renderer {
    constructor(emulatorScreen) {
        this.slots = [
            document.getElementById('slot0'),
            document.getElementById('slot1'),
        ];
        this.library = document.getElementById('library');
        this.trash = document.getElementById('trash');
        this.canvas = document.getElementById('screen');
        this.canvas.width = emulatorScreen.width;
        this.canvas.height = emulatorScreen.height;
        this.canvasCtx = this.canvas.getContext('2d');
        this.screenImageData = this.canvasCtx.createImageData(emulatorScreen.width, emulatorScreen.height);
        this.screenPixelCount = emulatorScreen.width * emulatorScreen.height;
    }
    renderInit(app) {
        this.trash.ondragover = e => e.preventDefault();
        this.trash.ondrop = e => {
            e.preventDefault();
            app.droppedOnTrash();
        };
        this.library.ondragover = e => e.preventDefault();
        this.library.ondrop = e => {
            var _a, _b;
            e.preventDefault();
            let file = null;
            if ((_a = e.dataTransfer) === null || _a === void 0 ? void 0 : _a.items) {
                for (let i = 0; i < e.dataTransfer.items.length; i++) {
                    const item = e.dataTransfer.items[i];
                    if (item.kind === 'file') {
                        file = item.getAsFile();
                        break;
                    }
                }
            }
            else if ((_b = e.dataTransfer) === null || _b === void 0 ? void 0 : _b.files) {
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
                file.slice(0, diskSize).arrayBuffer().then((buffer) => {
                    const toCopy = Math.min(disk.data.length, buffer.byteLength);
                    new Uint8Array(disk.data.buffer, 0, toCopy).set(new Uint8Array(buffer, 0, toCopy));
                    app.importDisk(disk);
                });
            }
            else {
                app.droppedOnLibrary();
            }
        };
    }
    renderLibrary(app, disks) {
        this.library.innerHTML = '';
        for (const disk of disks) {
            const elem = this.createLibraryDiskElement(app, disk);
            this.library.appendChild(elem);
        }
    }
    createLibraryDiskElement(app, disk) {
        const d = document.createElement('div');
        if (disk.state === 'deleting' || disk.state === 'saving') {
            d.className = 'disk diskLike inProgressDisk';
        }
        else if (disk.state === 'failed') {
            d.className = 'disk diskLike failedDisk';
        }
        else {
            d.className = 'disk diskLike';
        }
        this.fillSlotElement(app, d, { disk: disk.disk, place: 'library' }, disk.canInteract());
        return d;
    }
    fillSlotElement(app, slot, disk, allowInteractions) {
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
    renderSlots(app, slots) {
        for (let i = 0; i < 2; i++) {
            let slot = slots[i];
            this.slots[i].className = 'slot diskLike';
            if (slot === null) {
                this.prepEmptySlot(app, this.slots[i], i);
            }
            else {
                this.fillSlotElement(app, this.slots[i], { disk: slot.disk, place: 'slot' }, true);
            }
        }
    }
    updateInsertedDiskIndicators(slots) {
        for (let i = 0; i < 2; i++) {
            let slot = slots[i];
            if (slot !== null) {
                const workingIndicator = this.slots[i].firstChild;
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
    prepEmptySlot(app, slot, index) {
        slot.ondblclick = null;
        slot.ondragstart = null;
        slot.ondragend = null;
        slot.innerHTML = '';
        slot.draggable = false;
        slot.ondblclick = () => {
            let label = prompt('Enter new disk name');
            if (label !== null) {
                app.createDiskInSlot(index, label);
            }
        };
        slot.ondragover = e => e.preventDefault();
        slot.ondrop = e => {
            e.preventDefault();
            app.droppedOnSlot(index);
        };
    }
    renderScreen(buffer) {
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
function naturalCompare(a, b) {
    function digitCount(s, idx) {
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
        const cb = b[ib];
        if (ca >= '0' && ca <= '9' && cb >= '0' && cb <= '9') {
            const da = digitCount(a, ia);
            const na = parseInt(a.substr(ia, da));
            ia += da;
            const db = digitCount(b, ib);
            const nb = parseInt(b.substr(ib, db));
            ib += db;
            if (na < nb) {
                return -1;
            }
            else if (na > nb) {
                return 1;
            }
        }
        else if (ca < cb) {
            return -1;
        }
        else if (ca > cb) {
            return 1;
        }
        else {
            ia += 1;
            ib += 1;
        }
    }
    return 0;
}
function createUuid() {
    let dt = new Date().getTime();
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, c => {
        const r = (dt + Math.random() * 16) % 16 | 0;
        dt = Math.floor(dt / 16);
        return (c === 'x' ? r : (r & 0x3 | 0x8)).toString(16);
    });
}
//# sourceMappingURL=index.js.map