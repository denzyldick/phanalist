<?php

// Invalid: calling a method on a property of self that holds a foreign object
class UserController
{
    private UserRepository $userRepo;

    public function show(int $id): void
    {
        // Violation: $this->userRepo is a property holding a foreign object.
        // Calling find() on it, then getName() on the result — two levels deep.
        $name = $this->userRepo->find($id)->getName();
    }
}

class UserRepository
{
    public function find(int $id): User
    {
        return new User();
    }
}

class User
{
    public function getName(): string
    {
        return 'Alice';
    }
}
