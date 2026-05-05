<?php

class InvoiceService
{
    private InvoiceRepository $invoices;
    private Clock $clock;

    public function __construct(InvoiceRepository $invoices, Clock $clock)
    {
        $this->invoices = $invoices;
        $this->clock = $clock;
    }

    public function create(Invoice $invoice): Invoice
    {
        return $this->invoices->save($invoice);
    }
}
