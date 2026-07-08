let regexpConstructor = RegExp;
let execDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, "exec");
let testDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, "test");
let literal = /a+/g;
let first = literal.exec("baaa");
let second = literal.exec("zz");
let sticky = /a/y;
sticky.lastIndex = 1;
let stickyMatch = sticky.exec("ba");

let receiverError = false;
try {
  RegExp.prototype.test.call({}, "x");
} catch (error) {
  receiverError = error.name === "TypeError";
}

let descriptorOk =
  regexpConstructor === RegExp &&
  RegExp.name === "RegExp" &&
  RegExp.length === 2 &&
  RegExp.prototype.constructor === RegExp &&
  RegExp.prototype.exec.name === "exec" &&
  RegExp.prototype.exec.length === 1 &&
  execDescriptor.writable === true &&
  execDescriptor.enumerable === false &&
  execDescriptor.configurable === true &&
  RegExp.prototype.test.name === "test" &&
  RegExp.prototype.test.length === 1 &&
  testDescriptor.writable === true &&
  testDescriptor.enumerable === false &&
  testDescriptor.configurable === true;

let execOk =
  first[0] === "aaa" &&
  first.index === 1 &&
  first.input === "baaa" &&
  first.length === 1 &&
  literal.lastIndex === 0 &&
  second === null &&
  stickyMatch[0] === "a" &&
  stickyMatch.index === 1 &&
  sticky.lastIndex === 2;

let patternOk =
  new RegExp("foo", "i").test("FOO") &&
  /^foo/m.test("bar\nfoo") &&
  /./s.test("\n") &&
  /\d+/.exec("id=123")[0] === "123" &&
  /\w+/.exec("++abc")[0] === "abc" &&
  /[abc]+/.exec("zzcab")[0] === "cab";

if (!descriptorOk || !execOk || !patternOk || !receiverError) {
  throw new Test262Error("RegExp baseline behavior was unexpected");
}

42
