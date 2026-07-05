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

42;
