let fn = function namedCamera(left, right) {
    return left + right;
};
fn.alpha = 1;
fn["beta"] = 2;
fn.alpha += 40;
fn.count = fn(20, 22);

if (fn.alpha !== 41 || fn.beta !== 2 || fn.count !== 42) {
    throw new Test262Error("custom function properties were unexpected");
}

if (!("alpha" in fn) || !("beta" in fn) || !("length" in fn) || "missing" in fn) {
    throw new Test262Error("custom function property membership was unexpected");
}

delete fn.alpha;
fn.gamma = 3;
fn.alpha = 10;

let seen = "";
for (let key in fn) {
    seen = seen + key + ":" + fn[key] + ";";
}
if (seen !== "beta:2;count:42;gamma:3;alpha:10;") {
    throw new Test262Error("custom function property enumeration was unexpected");
}

delete fn.beta;
if ("beta" in fn || fn.beta !== undefined) {
    throw new Test262Error("custom function property deletion was unexpected");
}

fn.length = 99;
fn.name = "changed";
if (fn.length !== 2 || fn.name !== "namedCamera") {
    throw new Test262Error("function metadata assignment was unexpected");
}

42
