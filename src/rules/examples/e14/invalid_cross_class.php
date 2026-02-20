<?php

// Scenario 1: Injected dependency with chaining on result of a method call
class OrderService
{
    private CustomerRepository $customerRepo;

    public function process(int $orderId): void
    {
        // Violation: getCustomer() returns a Customer (unknown type to OrderService),
        // so chaining getName() on it is a LoD violation.
        $name = $this->customerRepo->getCustomer($orderId)->getName();
    }
}

// Scenario 2: Local variable receiving an object from another scope
class Importer
{
    public function run(): void
    {
        $builder = new Builder();
        // Violation: query() returns something unknown (Builder not in this class)
        $result = $builder->query()->execute();
    }
}

class Builder
{
    public function query(): Query { return new Query(); }
}

class Query
{
    public function execute(): array { return []; }
}
