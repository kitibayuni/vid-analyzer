extern "C" {
	std::vector<cv::barcode::BarcodeType>* std_vectorLcv_barcode_BarcodeTypeG_new_const() {
			std::vector<cv::barcode::BarcodeType>* ret = new std::vector<cv::barcode::BarcodeType>();
			return ret;
	}

	void std_vectorLcv_barcode_BarcodeTypeG_delete(std::vector<cv::barcode::BarcodeType>* instance) {
			delete instance;
	}

	size_t std_vectorLcv_barcode_BarcodeTypeG_len_const(const std::vector<cv::barcode::BarcodeType>* instance) {
			size_t ret = instance->size();
			return ret;
	}

	bool std_vectorLcv_barcode_BarcodeTypeG_isEmpty_const(const std::vector<cv::barcode::BarcodeType>* instance) {
			bool ret = instance->empty();
			return ret;
	}

	size_t std_vectorLcv_barcode_BarcodeTypeG_capacity_const(const std::vector<cv::barcode::BarcodeType>* instance) {
			size_t ret = instance->capacity();
			return ret;
	}

	void std_vectorLcv_barcode_BarcodeTypeG_shrinkToFit(std::vector<cv::barcode::BarcodeType>* instance) {
			instance->shrink_to_fit();
	}

	void std_vectorLcv_barcode_BarcodeTypeG_reserve_size_t(std::vector<cv::barcode::BarcodeType>* instance, size_t additional) {
			instance->reserve(instance->size() + additional);
	}

	void std_vectorLcv_barcode_BarcodeTypeG_remove_size_t(std::vector<cv::barcode::BarcodeType>* instance, size_t index) {
			instance->erase(instance->begin() + index);
	}

	void std_vectorLcv_barcode_BarcodeTypeG_swap_size_t_size_t(std::vector<cv::barcode::BarcodeType>* instance, size_t index1, size_t index2) {
			std::swap((*instance)[index1], (*instance)[index2]);
	}

	void std_vectorLcv_barcode_BarcodeTypeG_clear(std::vector<cv::barcode::BarcodeType>* instance) {
			instance->clear();
	}

	void std_vectorLcv_barcode_BarcodeTypeG_push_const_BarcodeType(std::vector<cv::barcode::BarcodeType>* instance, const cv::barcode::BarcodeType val) {
			instance->push_back(val);
	}

	void std_vectorLcv_barcode_BarcodeTypeG_insert_size_t_const_BarcodeType(std::vector<cv::barcode::BarcodeType>* instance, size_t index, const cv::barcode::BarcodeType val) {
			instance->insert(instance->begin() + index, val);
	}

	void std_vectorLcv_barcode_BarcodeTypeG_get_const_size_t(const std::vector<cv::barcode::BarcodeType>* instance, size_t index, cv::barcode::BarcodeType* ocvrs_return) {
			cv::barcode::BarcodeType ret = (*instance)[index];
			*ocvrs_return = ret;
	}

	void std_vectorLcv_barcode_BarcodeTypeG_set_size_t_const_BarcodeType(std::vector<cv::barcode::BarcodeType>* instance, size_t index, const cv::barcode::BarcodeType val) {
			(*instance)[index] = val;
	}

	std::vector<cv::barcode::BarcodeType>* std_vectorLcv_barcode_BarcodeTypeG_clone_const(const std::vector<cv::barcode::BarcodeType>* instance) {
			std::vector<cv::barcode::BarcodeType> ret = std::vector<cv::barcode::BarcodeType>(*instance);
			return new std::vector<cv::barcode::BarcodeType>(ret);
	}

	const cv::barcode::BarcodeType* std_vectorLcv_barcode_BarcodeTypeG_data_const(const std::vector<cv::barcode::BarcodeType>* instance) {
			const cv::barcode::BarcodeType* ret = instance->data();
			return ret;
	}

	cv::barcode::BarcodeType* std_vectorLcv_barcode_BarcodeTypeG_dataMut(std::vector<cv::barcode::BarcodeType>* instance) {
			cv::barcode::BarcodeType* ret = instance->data();
			return ret;
	}

	std::vector<cv::barcode::BarcodeType>* cv_fromSlice_const_const_BarcodeTypeX_size_t(const cv::barcode::BarcodeType* data, size_t len) {
			return new std::vector<cv::barcode::BarcodeType>(data, data + len);
	}

}


