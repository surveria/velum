class Counter {
  #value = 1;

  #increment(step) {
    this.#value += step;
    return this.#value;
  }

  get #current() {
    return this.#value;
  }

  set #current(value) {
    this.#value = value;
  }

  update(value) {
    this.#current = value;
    return this.#increment(2) + ":" + this.#current;
  }

  has(value) {
    return #value in value;
  }

  static #count = 0;

  static next() {
    return ++this.#count;
  }
}

var counter = new Counter();
print(counter.update(40), counter.has(counter), counter.has({}));
print(Counter.next(), Counter.next());

function makeBox() {
  return class {
    #value = 9;

    has(value) {
      return #value in value;
    }
  };
}

var First = makeBox();
var Second = makeBox();
var first = new First();
var second = new Second();
print(first.has(first), first.has(second), second.has(second));
