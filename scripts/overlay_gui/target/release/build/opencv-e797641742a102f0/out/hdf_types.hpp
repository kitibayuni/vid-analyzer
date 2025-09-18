extern "C" {
	const cv::hdf::HDF5* cv_PtrLcv_hdf_HDF5G_getInnerPtr_const(const cv::Ptr<cv::hdf::HDF5>* instance) {
			return instance->get();
	}

	cv::hdf::HDF5* cv_PtrLcv_hdf_HDF5G_getInnerPtrMut(cv::Ptr<cv::hdf::HDF5>* instance) {
			return instance->get();
	}

	cv::Ptr<cv::hdf::HDF5>* cv_PtrLcv_hdf_HDF5G_new_null_const() {
			return new cv::Ptr<cv::hdf::HDF5>();
	}

	void cv_PtrLcv_hdf_HDF5G_delete(cv::Ptr<cv::hdf::HDF5>* instance) {
			delete instance;
	}

}

