let raw = String.raw({ raw: { 0: "a", 1: "b", 2: "c", length: 3 } }, 1, 2);
let boxed = new String("Boxed");

let staticOk = String.fromCharCode(67, 97, 109) === "Cam" &&
    String.fromCharCode(65.9, -1) === "A\uFFFF" &&
    String.fromCodePoint(0x2603).codePointAt(0) === 0x2603 &&
    raw === "a1b2c";

let prototypeOk = "camera".at(0) === "c" &&
    "camera".at(-1) === "a" &&
    "camera".at(99) === undefined &&
    "snow\u2603".codePointAt(4) === 0x2603 &&
    "cam".padStart(5, "0") === "00cam" &&
    "cam".padEnd(6, "ab") === "camaba" &&
    "cam".padStart(2, "0") === "cam" &&
    "cam".padEnd(6, "") === "cam" &&
    "\ttrim\n".trimLeft() === "trim\n" &&
    "\ttrim\n".trimRight() === "\ttrim" &&
    "MiXeD".toLocaleLowerCase() === "mixed" &&
    "MiXeD".toLocaleUpperCase() === "MIXED" &&
    String.prototype.toString.call(boxed) === "Boxed" &&
    String.prototype.valueOf.call(boxed) === "Boxed";

let metadataOk = String.fromCharCode.length === 1 &&
    String.fromCodePoint.length === 1 &&
    String.raw.length === 1 &&
    String.prototype.at.length === 1 &&
    String.prototype.codePointAt.length === 1 &&
    String.prototype.padStart.length === 1 &&
    String.prototype.padEnd.length === 1 &&
    String.prototype.toString.length === 0 &&
    String.prototype.valueOf.length === 0 &&
    String.prototype.trimLeft === String.prototype.trimStart &&
    String.prototype.trimRight === String.prototype.trimEnd;

if (!staticOk || !prototypeOk || !metadataOk) {
    throw new Test262Error("String static unicode methods behavior was unexpected");
}

assert.throws(RangeError, function() {
    String.fromCodePoint(0x110000);
});

assert.throws(TypeError, function() {
    String.prototype.toString.call(42);
});

42
