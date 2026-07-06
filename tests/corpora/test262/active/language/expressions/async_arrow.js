let plusOne = value => value + 1;

let answer = async (left, right,) => {
    let base = await Promise.resolve(left);
    return plusOne(base + right);
};

await answer(39, 2);
