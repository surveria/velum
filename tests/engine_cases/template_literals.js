let value = 0;
let empty = ``;
let text = `camera`;
let escaped = `\`\$\\`;
let lines = `front
door`;

if (empty === "") {
  value = value + 10;
}
if (text === "camera") {
  value = value + 10;
}
if (escaped === "`$\\") {
  value = value + 10;
}
if (lines === "front\ndoor") {
  value = value + 12;
}

value;
