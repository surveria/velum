let empty = ``;
let text = `camera`;
let escaped = `\`\$\\`;
let lines = `front
door`;

if (empty !== "") {
  throw new Test262Error("empty template literal mismatch");
}
if (text !== "camera") {
  throw new Test262Error("template literal text mismatch");
}
if (escaped !== "`$\\") {
  throw new Test262Error("template literal escape mismatch");
}
if (lines !== "front\ndoor") {
  throw new Test262Error("template literal line terminator mismatch");
}

let count = 5;
let substituted = `count=${count}, twice=${count * 2}`;
if (substituted !== "count=5, twice=10") {
  throw new Test262Error("template literal substitution mismatch");
}

let adjacent = `${1}${""}${2}${3}`;
if (adjacent !== "123") {
  throw new Test262Error("adjacent template substitutions mismatch");
}

let nested = `outer ${`inner ${count + 37}`} end`;
if (nested !== "outer inner 42 end") {
  throw new Test262Error("nested template literal mismatch");
}

let braces = `object ${ {answer: count} } end`;
if (braces !== "object [object Object] end") {
  throw new Test262Error("template substitution object braces mismatch");
}

let primitives = `${undefined}:${null}:${true}:${false}`;
if (primitives !== "undefined:null:true:false") {
  throw new Test262Error("template substitution primitive conversion mismatch");
}

let escapedSubstitution = `keep \${count} raw`;
if (escapedSubstitution !== "keep ${count} raw") {
  throw new Test262Error("escaped template substitution mismatch");
}

function label(name) {
  return "<" + name + ">";
}
let called = `call ${label("x")} pick ${count > 1 ? "yes" : "no"}`;
if (called !== "call <x> pick yes") {
  throw new Test262Error("template substitution call mismatch");
}

let symbolError = "";
try {
  `${Symbol("marker")}`;
} catch (error) {
  if (error instanceof TypeError) {
    symbolError = "TypeError";
  }
}
if (symbolError !== "TypeError") {
  throw new Test262Error("symbol template substitution must throw TypeError");
}

42;
