<?php

// Invalid: null-safe chaining on getCustomer() which returns a foreign type
class InvoiceService
{
    public function getCustomer(): Customer
    {
        return new Customer();
    }

    public function process(): void
    {
        // Violation: getCustomer() returns Customer (not self), so ?->getName() is a LoD violation
        $name = $this->getCustomer()?->getName();
    }
}

class Customer
{
    public function getName(): string
    {
        return 'Alice';
    }
}
