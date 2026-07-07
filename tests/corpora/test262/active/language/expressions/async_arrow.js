let plusOne = value => value + 1;

let answer = async (left, right,) => {
    let base = await Promise.resolve(left);
    return plusOne(base + right);
};

await answer(39, 2);

let defaultAnswer = async (left = 40, right = 2,) => {
    let base = await Promise.resolve(left);
    return plusOne(base + right - 1);
};

await defaultAnswer(undefined);
