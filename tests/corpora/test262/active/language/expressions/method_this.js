let camera = { value: 40, name: "front" };
camera.read = function(delta) {
    return this.value + delta;
};
camera.write = function(value) {
    this.value = value;
    return this.read(0);
};

let first = camera.read(2);
let second = camera["write"](42);
let parenthesized = (camera.read)(0);
let keywordProperty = { this: "keyword" }.this;

if (first !== 42) {
    throw new Test262Error("member method call did not bind this");
}
if (second !== 42) {
    throw new Test262Error("computed method call did not bind this");
}
if (parenthesized !== 42) {
    throw new Test262Error("parenthesized method call did not bind this");
}
if (camera.value !== 42) {
    throw new Test262Error("method call did not update receiver state");
}
if (keywordProperty !== "keyword") {
    throw new Test262Error("this keyword property name was unexpected");
}

42
