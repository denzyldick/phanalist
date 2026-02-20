<?php

// Invalid: Chaining on a method that returns a different type (LoD violation)
class Order
{
    public function getCustomer(): Customer
    {
        return new Customer();
    }

    public function process(): void
    {
        // Violation: getCustomer() returns Customer (not Order),
        // so calling getName() on it is talking to a stranger.
        $name = $this->getCustomer()->getName();
    }
}

class Customer
{
    public function getName(): string
    {
        return 'Alice';
    }
}
