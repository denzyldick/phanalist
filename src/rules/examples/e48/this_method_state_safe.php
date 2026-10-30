<?php

final class Container
{
    private bool $registered = false;

    public function resolve($id): void
    {
        $this->register($id);

        $this->resolve($id);
    }

    private function register($id): void
    {
        $this->registered = true;
    }
}
