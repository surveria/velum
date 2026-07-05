let fn = function namedCamera(left, right) {
    return left + right;
};
fn.alpha = 1;
fn["beta"] = 2;
fn.alpha += 40;
fn.count = fn(20, 22);

print(fn.alpha, fn.beta, fn.count, fn.length, fn.name);
print("alpha" in fn, "beta" in fn, "length" in fn, "missing" in fn);

delete fn.alpha;
fn.gamma = 3;
fn.alpha = 10;

let seen = "";
for (let key in fn) {
    seen = seen + key + ":" + fn[key] + ";";
}
print(seen);

delete fn.beta;
print("beta" in fn, fn.beta === undefined);

fn.length = 99;
fn.name = "changed";
print(fn.length, fn.name);
