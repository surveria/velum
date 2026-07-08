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
