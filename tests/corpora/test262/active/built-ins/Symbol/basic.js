let first = Symbol("slot");
let second = Symbol("slot");
let object = {};
object[first] = 7;
object[second] = 9;

let descriptor = Object.getOwnPropertyDescriptor(object, first);
let keys = Object.keys(object);
let iteratorDescriptor = Object.getOwnPropertyDescriptor(Symbol, "iterator");
let tagged = {};
tagged[Symbol.toStringTag] = "tagged";

typeof Symbol === "function" &&
    Symbol.name === "Symbol" &&
    Symbol.length === 0 &&
    typeof first === "symbol" &&
    String(first) === "Symbol(slot)" &&
    first !== second &&
    object[first] === 7 &&
    object[second] === 9 &&
    Object.hasOwn(object, first) === true &&
    Object.hasOwn(object, second) === true &&
    descriptor.value === 7 &&
    descriptor.enumerable === true &&
    descriptor.writable === true &&
    descriptor.configurable === true &&
    keys.length === 0 &&
    typeof Symbol.iterator === "symbol" &&
    Symbol.iterator === Symbol.iterator &&
    Symbol.iterator !== Symbol.toStringTag &&
    iteratorDescriptor.value === Symbol.iterator &&
    iteratorDescriptor.enumerable === false &&
    iteratorDescriptor.writable === false &&
    iteratorDescriptor.configurable === false &&
    tagged[Symbol.toStringTag] === "tagged" ? 42 : 0;
