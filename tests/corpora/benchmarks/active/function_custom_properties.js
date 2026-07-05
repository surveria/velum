let total = 0;
let fn = function camera(left, right) {
    return left + right;
};

fn.alpha = 1;
fn.beta = 2;
fn.gamma = 3;

for (let index = 0; index < 128; index++) {
    fn.alpha += 1;
    total += fn.alpha;
    total += fn.beta;
    if ("gamma" in fn) {
        total += fn.gamma;
    }
    for (let key in fn) {
        total += fn[key];
    }
}

total
