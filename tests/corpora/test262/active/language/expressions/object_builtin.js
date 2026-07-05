let plain = {};
let objectConstructor = Object;
let created = Object();
let constructed = new Object();
let returned = Object(plain);
let returnedFromNew = new Object(plain);
let originalPrototype = Object.prototype;
Object.prototype = null;
let prototypeStayed = Object.prototype === originalPrototype &&
    (new Object()).__proto__ === originalPrototype;

objectConstructor === Object &&
    Object.prototype.__proto__ === null &&
    Object.prototype.constructor === Object &&
    Object.prototype.constructor.prototype === Object.prototype &&
    plain.constructor === Object &&
    created.__proto__ === Object.prototype &&
    constructed.__proto__ === Object.prototype &&
    returned === plain &&
    returnedFromNew === plain &&
    prototypeStayed &&
    typeof Object === "function" &&
    Object.name === "Object" &&
    Object.length === 1 ? 42 : 0
