<?php

class Foo {
    public function bar(): self {
        return $this;
    }
}

class UserRepository {
    public function save(): UserRepository {
        return $this;
    }
}

class UserService {
    private UserRepository $repo;

    public function __construct(UserRepository $repo) {
        $this->repo = $repo;
    }

    public function doSomething() {
        // Calling method on $this->repo -> it's allowed if repo is of type UserRepository
        // However, chaining on it multiple times should NOT trigger LoD if repo returns self
        $this->repo->save()->save();
    }
}
