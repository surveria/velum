let error = new TypeError("typed");
let aggregate = new AggregateError([], "many");

if (error.toString() !== "TypeError: typed") {
  throw new Test262Error("TypeError toString mismatch");
}
if (Object.getPrototypeOf(error) !== TypeError.prototype) {
  throw new Test262Error("TypeError instance prototype mismatch");
}
if (Object.getPrototypeOf(TypeError.prototype) !== Error.prototype) {
  throw new Test262Error("TypeError prototype parent mismatch");
}
if (TypeError.prototype.toString !== Error.prototype.toString) {
  throw new Test262Error("TypeError toString inheritance mismatch");
}
if (!(error instanceof Error)) {
  throw new Test262Error("TypeError should be an Error instance");
}
if (!(error instanceof TypeError)) {
  throw new Test262Error("TypeError should be a TypeError instance");
}
if (error instanceof SyntaxError) {
  throw new Test262Error("TypeError should not be a SyntaxError instance");
}
if (Error.prototype.toString.call({ name: "Custom", message: "message" }) !== "Custom: message") {
  throw new Test262Error("Error.prototype.toString object receiver mismatch");
}
if (Error.prototype.toString.call({ name: "", message: "message" }) !== "message") {
  throw new Test262Error("Error.prototype.toString empty name mismatch");
}
if (Error.prototype.toString.call({ name: "OnlyName", message: "" }) !== "OnlyName") {
  throw new Test262Error("Error.prototype.toString empty message mismatch");
}
if (aggregate.name !== "AggregateError") {
  throw new Test262Error("AggregateError name mismatch");
}
if (aggregate.message !== "many") {
  throw new Test262Error("AggregateError message mismatch");
}
if (AggregateError.length !== 2) {
  throw new Test262Error("AggregateError length mismatch");
}

42;
