let text = "Camera Stream";
let padded = " \tCamera Stream\n ";
let boxed = new String("Boxed");
let protoKeys = "";
for (let key in String.prototype) {
    protoKeys = protoKeys + key + ";";
}

let searchOk = text.charAt(0) === "C" &&
    text.charAt(-1) === "" &&
    text.charAt(99) === "" &&
    text.charCodeAt(1) === 97 &&
    text.charCodeAt(99) !== text.charCodeAt(99) &&
    text.includes("Stream") &&
    text.includes("mera", 2) &&
    !text.includes("Camera", 1) &&
    text.indexOf("a") === 1 &&
    text.indexOf("a", 2) === 5 &&
    text.indexOf("missing") === -1 &&
    text.lastIndexOf("a") === 11 &&
    text.lastIndexOf("a", 2) === 1 &&
    text.startsWith("Camera") &&
    text.startsWith("mera", 2) &&
    !text.startsWith("Camera", 1) &&
    text.endsWith("Stream") &&
    text.endsWith("Camera", 6) &&
    !text.endsWith("Camera", 5);

let sliceOk = text.slice(1, 6) === "amera" &&
    text.slice(-6, -1) === "Strea" &&
    text.slice(7) === "Stream" &&
    text.slice(8, 3) === "" &&
    text.substring(7, 13) === "Stream" &&
    text.substring(6, 0) === "Camera" &&
    text.substring(-4, 6) === "Camera";

let transformOk = "go".repeat(3) === "gogogo" &&
    "go".repeat(undefined) === "" &&
    "a".concat("b", 7, true) === "ab7true" &&
    padded.trim() === "Camera Stream" &&
    padded.trimStart() === "Camera Stream\n " &&
    padded.trimEnd() === " \tCamera Stream" &&
    "MiXeD".toLowerCase() === "mixed" &&
    "MiXeD".toUpperCase() === "MIXED";

let receiverOk = boxed.slice(1, 4) === "oxe" &&
    String.prototype.slice.call(12345, 1, 4) === "234" &&
    String.prototype.includes.call(true, "ru") &&
    String.prototype.concat.call("id-", 42) === "id-42";

let metadataOk = typeof String.prototype.slice === "function" &&
    String.prototype.slice.name === "slice" &&
    String.prototype.slice.length === 2 &&
    String.prototype.trim.length === 0 &&
    String.prototype.includes.length === 1 &&
    protoKeys === "";

if (!searchOk || !sliceOk || !transformOk || !receiverOk || !metadataOk) {
    throw new Test262Error("String.prototype method behavior was unexpected");
}

assert.throws(TypeError, function() {
    String.prototype.trim.call(null);
});
assert.throws(TypeError, function() {
    String.prototype.indexOf.call(undefined, "x");
});

42
