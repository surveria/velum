let regexpConstructor = RegExp;
let execDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, "exec");
let testDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, "test");
let toStringDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, "toString");
let symbolMatchDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, Symbol.match);
let symbolMatchAllDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, Symbol.matchAll);
let symbolReplaceDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, Symbol.replace);
let symbolSearchDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, Symbol.search);
let symbolSplitDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, Symbol.split);
let sourceDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, "source");
let flagsDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, "flags");
let literal = /a+/g;
let first = literal.exec("baaa");
let second = literal.exec("zz");
let sticky = /a/y;
sticky.lastIndex = 1;
let stickyMatch = sticky.exec("ba");
let searchRestore = /a+/g;
searchRestore.lastIndex = 2;
let matchAllPattern = /a/g;
matchAllPattern.lastIndex = 1;
let matchAllIterator = matchAllPattern[Symbol.matchAll]("aba");
let matchAllPrototype = Object.getPrototypeOf(matchAllIterator);
let matchAllNextDescriptor = Object.getOwnPropertyDescriptor(matchAllPrototype, "next");
let matchAllTagDescriptor = Object.getOwnPropertyDescriptor(matchAllPrototype, Symbol.toStringTag);
let matchAllFirst = matchAllIterator.next();
let matchAllSecond = matchAllIterator.next();
let matchAllNonGlobal = (/a/)[Symbol.matchAll]("aba");
let matchAllNonGlobalFirst = matchAllNonGlobal.next();
let matchAllNonGlobalSecond = matchAllNonGlobal.next();
let matchAllOk =
  Object.hasOwn(matchAllIterator, "next") === false &&
  matchAllPrototype.next.name === "next" &&
  matchAllPrototype.next.length === 0 &&
  matchAllNextDescriptor.writable === true &&
  matchAllNextDescriptor.enumerable === false &&
  matchAllNextDescriptor.configurable === true &&
  matchAllPrototype[Symbol.toStringTag] === "RegExp String Iterator" &&
  matchAllTagDescriptor.writable === false &&
  matchAllTagDescriptor.enumerable === false &&
  matchAllTagDescriptor.configurable === true &&
  Object.prototype.toString.call(matchAllIterator) === "[object RegExp String Iterator]" &&
  matchAllIterator[Symbol.iterator]() === matchAllIterator &&
  matchAllFirst.done === false &&
  matchAllFirst.value[0] === "a" &&
  matchAllFirst.value.index === 2 &&
  matchAllFirst.value.input === "aba" &&
  matchAllSecond.done === true &&
  matchAllPattern.lastIndex === 1 &&
  matchAllNonGlobalFirst.done === false &&
  matchAllNonGlobalFirst.value[0] === "a" &&
  matchAllNonGlobalFirst.value.index === 0 &&
  matchAllNonGlobalSecond.done === true;

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
  testDescriptor.configurable === true &&
  RegExp.prototype.toString.name === "toString" &&
  RegExp.prototype.toString.length === 0 &&
  toStringDescriptor.writable === true &&
  toStringDescriptor.enumerable === false &&
  toStringDescriptor.configurable === true &&
  RegExp.prototype[Symbol.match].name === "[Symbol.match]" &&
  RegExp.prototype[Symbol.match].length === 1 &&
  symbolMatchDescriptor.writable === true &&
  symbolMatchDescriptor.enumerable === false &&
  symbolMatchDescriptor.configurable === true &&
  RegExp.prototype[Symbol.matchAll].name === "[Symbol.matchAll]" &&
  RegExp.prototype[Symbol.matchAll].length === 1 &&
  symbolMatchAllDescriptor.writable === true &&
  symbolMatchAllDescriptor.enumerable === false &&
  symbolMatchAllDescriptor.configurable === true &&
  RegExp.prototype[Symbol.replace].name === "[Symbol.replace]" &&
  RegExp.prototype[Symbol.replace].length === 2 &&
  symbolReplaceDescriptor.writable === true &&
  symbolReplaceDescriptor.enumerable === false &&
  symbolReplaceDescriptor.configurable === true &&
  RegExp.prototype[Symbol.search].name === "[Symbol.search]" &&
  RegExp.prototype[Symbol.search].length === 1 &&
  symbolSearchDescriptor.writable === true &&
  symbolSearchDescriptor.enumerable === false &&
  symbolSearchDescriptor.configurable === true &&
  RegExp.prototype[Symbol.split].name === "[Symbol.split]" &&
  RegExp.prototype[Symbol.split].length === 2 &&
  symbolSplitDescriptor.writable === true &&
  symbolSplitDescriptor.enumerable === false &&
  symbolSplitDescriptor.configurable === true &&
  Object.hasOwn(literal, "source") === false &&
  Object.hasOwn(literal, "flags") === false &&
  sourceDescriptor.get.name === "get source" &&
  sourceDescriptor.get.length === 0 &&
  sourceDescriptor.set === undefined &&
  sourceDescriptor.enumerable === false &&
  sourceDescriptor.configurable === true &&
  flagsDescriptor.get.name === "get flags" &&
  flagsDescriptor.get.length === 0 &&
  flagsDescriptor.set === undefined &&
  flagsDescriptor.enumerable === false &&
  flagsDescriptor.configurable === true;

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
  RegExp(literal) === literal &&
  new RegExp(literal) !== literal &&
  new RegExp(literal).source === "a+" &&
  new RegExp(literal).flags === "g" &&
  new RegExp(literal, "mi").flags === "im" &&
  new RegExp("").source === "(?:)" &&
  new RegExp("/\n\r\u2028\u2029").source === "\\/\\n\\r\\u2028\\u2029" &&
  /a/gim.toString() === "/a/gim" &&
  new RegExp("").toString() === "/(?:)/" &&
  RegExp.prototype.toString.call({ source: "x", flags: "g" }) === "/x/g" &&
  (/a+/)[Symbol.match]("baaa")[0] === "aaa" &&
  (/a/g)[Symbol.match]("aba").join("-") === "a-a" &&
  (/z/)[Symbol.match]("aba") === null &&
  (/a+/)[Symbol.replace]("baaa", "x") === "bx" &&
  (/a/g)[Symbol.replace]("aba", "x") === "xbx" &&
  (/z/)[Symbol.replace]("aba", "x") === "aba" &&
  (/a+/)[Symbol.search]("baaa") === 1 &&
  searchRestore[Symbol.search]("baaa") === 1 &&
  searchRestore.lastIndex === 2 &&
  (/z/)[Symbol.search]("baaa") === -1 &&
  (/-/)[Symbol.split]("a-b-c").join("|") === "a|b|c" &&
  (/-/)[Symbol.split]("a-b-c", 2).join("|") === "a|b" &&
  matchAllOk &&
  /a/gim.source === "a" &&
  /a/gim.flags === "gim" &&
  /a/gim.global === true &&
  /a/gim.ignoreCase === true &&
  /a/gim.multiline === true &&
  /a/s.dotAll === true &&
  /a/u.unicode === true &&
  /a/y.sticky === true &&
  /a/d.hasIndices === true &&
  /a/v.unicodeSets === true &&
  /^foo/m.test("bar\nfoo") &&
  /./s.test("\n") &&
  /\d+/.exec("id=123")[0] === "123" &&
  /\w+/.exec("++abc")[0] === "abc" &&
  /[abc]+/.exec("zzcab")[0] === "cab";

if (!descriptorOk || !execOk || !patternOk || !receiverError) {
  throw new Test262Error("RegExp baseline behavior was unexpected");
}

42
