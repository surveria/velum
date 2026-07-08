let text = "id=123;id=456;name=abc";
let single = text.match(/\d+/);
let global = text.match(/\d+/g);
let adjacent = "a1b2".match(/\w/g);
let search = text.search(/name/);
let replacedFirst = text.replace(/\d+/, "N");
let replacedGlobal = text.replace(/\d+/g, "N");
let splitGlobal = text.split(/;/g);
let splitFirst = text.split(/;/);

let metadataOk =
    String.prototype.match.name === "match" &&
    String.prototype.match.length === 1 &&
    String.prototype.search.name === "search" &&
    String.prototype.search.length === 1 &&
    String.prototype.replace.name === "replace" &&
    String.prototype.replace.length === 2 &&
    String.prototype.split.name === "split" &&
    String.prototype.split.length === 2;

let behaviorOk =
    single[0] === "123" &&
    single.index === 3 &&
    global.length === 2 &&
    global[0] === "123" &&
    global[1] === "456" &&
    adjacent.length === 4 &&
    adjacent[0] === "a" &&
    adjacent[3] === "2" &&
    search === 14 &&
    replacedFirst === "id=N;id=456;name=abc" &&
    replacedGlobal === "id=N;id=N;name=abc" &&
    splitGlobal.length === 3 &&
    splitGlobal[0] === "id=123" &&
    splitGlobal[2] === "name=abc" &&
    splitFirst.length === 3 &&
    splitFirst[0] === "id=123" &&
    splitFirst[1] === "id=456" &&
    splitFirst[2] === "name=abc";

if (!metadataOk || !behaviorOk) {
    throw new Test262Error("String RegExp interop behavior was unexpected");
}

42
