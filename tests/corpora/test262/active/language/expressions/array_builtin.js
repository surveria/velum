let early = [];
let arrayConstructor = Array;
let created = Array();
let constructed = new Array();
let withElements = Array("front", 42);
let withLength = Array(3);
let originalPrototype = Array.prototype;
Array.prototype = null;
let prototypeStayed = Array.prototype === originalPrototype &&
    [].__proto__ === originalPrototype;

arrayConstructor === Array &&
    typeof Array === "function" &&
    Array.name === "Array" &&
    Array.length === 1 &&
    Array.prototype.__proto__ === Object.prototype &&
    Array.prototype.constructor === Array &&
    Array.prototype.constructor.prototype === Array.prototype &&
    early.constructor === Array &&
    early.__proto__ === Array.prototype &&
    created.__proto__ === Array.prototype &&
    constructed.__proto__ === Array.prototype &&
    created.length === 0 &&
    constructed.length === 0 &&
    withElements.length === 2 &&
    withElements[0] === "front" &&
    withElements[1] === 42 &&
    withLength.length === 3 &&
    withLength[0] === undefined &&
    prototypeStayed ? 42 : 0
