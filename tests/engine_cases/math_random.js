let first = Math.random();
let second = Math.random();

let metadataOk =
    Math.random.name === "random" &&
    Math.random.length === 0;

let typeOk =
    typeof first === "number" &&
    typeof second === "number" &&
    first === first &&
    second === second;

let rangeOk =
    first >= 0 &&
    first < 1 &&
    second >= 0 &&
    second < 1;

print(metadataOk, typeOk, rangeOk);

metadataOk && typeOk && rangeOk ? 42 : 0
