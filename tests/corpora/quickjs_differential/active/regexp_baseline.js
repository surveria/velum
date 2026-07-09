let global = /a+/g;
let first = global.exec("baaa");
let second = global.exec("zz");
let sticky = /a/y;
sticky.lastIndex = 1;
let stickyMatch = sticky.exec("ba");

print(RegExp.name, RegExp.length, RegExp.prototype.exec.name, RegExp.prototype.test.length);
print(first[0], first.index, first.input, first.length, global.lastIndex, second === null);
print(stickyMatch[0], stickyMatch.index, sticky.lastIndex);
print(new RegExp("foo", "i").test("FOO"), /^foo/m.test("bar\nfoo"), /./s.test("\n"));
print(/\d+/.exec("id=123")[0], /\w+/.exec("++abc")[0], /[abc]+/.exec("zzcab")[0]);
let sourceDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, "source");
let flagsDescriptor = Object.getOwnPropertyDescriptor(RegExp.prototype, "flags");
let cloned = new RegExp(global, "mi");
print(Object.hasOwn(global, "source"), sourceDescriptor.get.name, sourceDescriptor.enumerable, sourceDescriptor.configurable);
print(flagsDescriptor.get.name, flagsDescriptor.enumerable, flagsDescriptor.configurable);
print(RegExp(global) === global, new RegExp(global) === global, cloned.source, cloned.flags);
print(new RegExp("").source, new RegExp("/\n\r").source);
print(/a/gim.source, /a/gim.flags, /a/gim.global, /a/gim.ignoreCase, /a/gim.multiline);
print(/a/s.dotAll, /a/u.unicode, /a/y.sticky);
print(/a/gim.toString(), new RegExp("").toString(), RegExp.prototype.toString.call({ source: "x", flags: "g" }));
print(RegExp.prototype.toString.name, RegExp.prototype.toString.length);
print(RegExp.prototype[Symbol.match].name, RegExp.prototype[Symbol.match].length);
print(RegExp.prototype[Symbol.matchAll].name, RegExp.prototype[Symbol.matchAll].length);
print(RegExp.prototype[Symbol.replace].name, RegExp.prototype[Symbol.replace].length);
print(RegExp.prototype[Symbol.search].name, RegExp.prototype[Symbol.search].length);
print(RegExp.prototype[Symbol.split].name, RegExp.prototype[Symbol.split].length);
print((/a+/)[Symbol.match]("baaa")[0], (/a/g)[Symbol.match]("aba").join("-"), (/z/)[Symbol.match]("aba") === null);
print((/a+/)[Symbol.replace]("baaa", "x"), (/a/g)[Symbol.replace]("aba", "x"), (/z/)[Symbol.replace]("aba", "x"));
let searchPattern = /a+/g;
searchPattern.lastIndex = 2;
print(searchPattern[Symbol.search]("baaa"), searchPattern.lastIndex, (/z/)[Symbol.search]("baaa"));
print((/-/)[Symbol.split]("a-b-c").join("|"), (/-/)[Symbol.split]("a-b-c", 2).join("|"));
let matchAllPattern = /a/g;
matchAllPattern.lastIndex = 1;
let matchAllIterator = matchAllPattern[Symbol.matchAll]("aba");
let matchAllPrototype = Object.getPrototypeOf(matchAllIterator);
let matchAllTagDescriptor = Object.getOwnPropertyDescriptor(matchAllPrototype, Symbol.toStringTag);
let matchAllFirst = matchAllIterator.next();
let matchAllSecond = matchAllIterator.next();
let matchAllNonGlobal = (/a/)[Symbol.matchAll]("aba");
let matchAllNonGlobalFirst = matchAllNonGlobal.next();
let matchAllNonGlobalSecond = matchAllNonGlobal.next();
print(
  Object.hasOwn(matchAllIterator, "next"),
  matchAllPrototype.next.name,
  matchAllPrototype.next.length,
  matchAllPrototype[Symbol.toStringTag],
  matchAllTagDescriptor.writable,
  Object.prototype.toString.call(matchAllIterator)
);
print(
  matchAllIterator[Symbol.iterator]() === matchAllIterator,
  matchAllFirst.done,
  matchAllFirst.value[0],
  matchAllFirst.value.index,
  matchAllSecond.done,
  matchAllPattern.lastIndex
);
print(
  matchAllNonGlobalFirst.done,
  matchAllNonGlobalFirst.value[0],
  matchAllNonGlobalFirst.value.index,
  matchAllNonGlobalSecond.done
);
